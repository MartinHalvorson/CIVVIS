//! `civvis core-probe`: which parts of a saved world does the game itself read?
//!
//! A deterministic game is a function `next(state, action) -> state`. Its
//! irreducible state is the smallest part of the save that function reads.
//! This measures that part directly instead of arguing about it:
//!
//! 1. `record` plays a seeded game with the stock AI to turn T, saves it,
//!    plays K more turns, and keeps every action applied in that window and
//!    the world the window ended on.
//! 2. `ablate` blanks one field of the turn-T save at a time, applies exactly
//!    the recorded actions with no AI in the loop, and reports which other
//!    fields of the end state came out different. A field the transition
//!    function never reads changes nothing else. A field it does read changes
//!    something, or gets actions refused.
//! 3. `strip` blanks a whole list of fields at once, which is the test of
//!    sufficiency: whatever is left must still replay to the same world.
//!
//! Every answer is per window and per seed. "Nothing else changed" means
//! nothing changed in these windows, which is evidence, not proof.

use std::collections::BTreeSet;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::time::Instant;

use civvis::ai::{AdvancedAi, Ai};
use civvis::game::{Action, Game, GameOptions};
use civvis::setup::{MapScript, MapSize};
use civvis::shared_sort::SharedSort;
use serde_json::{Map, Value};

pub fn command(args: Vec<String>) {
    // Panics are expected (blanking a city's owner panics somewhere) and are
    // part of the measurement, so keep their messages out of the report.
    std::panic::set_hook(Box::new(|info| {
        if info
            .location()
            .is_some_and(|l| l.file().ends_with("core_probe.rs"))
        {
            eprintln!("core-probe: {info}");
        }
    }));
    match args.get(1).map(String::as_str) {
        Some("record") => record(&args[2..]),
        Some("ablate") => ablate(&args[2..]),
        Some("strip") => strip(&args[2..]),
        Some("bench") => bench(&args[2..]),
        Some("agent") => agent(&args[2..]),
        Some("blank") => blank_file(&args[2..]),
        Some("diff") => diff_files(&args[2..]),
        _ => {
            eprintln!(
                "usage: civvis core-probe record <players> <seed> <T> <K> <dir> [budget-seconds] [fog]\n       \
                 civvis core-probe ablate <dir> <deadline-seconds>\n       \
                 civvis core-probe strip <dir> <paths-file>\n       \
                 civvis core-probe bench <save.json> [iterations]"
            );
            std::process::exit(2);
        }
    }
}

fn read(path: &str) -> String {
    std::fs::read_to_string(path).unwrap_or_else(|e| panic!("read {path}: {e}"))
}

fn write(path: &str, text: &str) {
    std::fs::write(path, text).unwrap_or_else(|e| panic!("write {path}: {e}"));
}

fn record(args: &[String]) {
    let players: usize = args[0].parse().unwrap();
    let seed: u64 = args[1].parse().unwrap();
    let t: u32 = args[2].parse().unwrap();
    let k: u32 = args[3].parse().unwrap();
    let dir = &args[4];
    // A slow host shortens the game rather than losing it: past this many
    // seconds the save is taken at the next turn boundary instead of at T.
    let budget_s: f64 = args.get(5).map(|s| s.parse().unwrap()).unwrap_or(1e9);
    let budget_end = Instant::now() + std::time::Duration::from_secs_f64(budget_s);
    std::fs::create_dir_all(dir).unwrap();
    let size = MapSize::for_players(players);
    let mut opts = GameOptions::new(
        players,
        size.width,
        size.height,
        seed,
        250,
        size.default_city_states,
    );
    // Online's clock moves the eras twice as fast per turn, so a short
    // window reaches the late-game systems (congress, corporations, the
    // space race) a Standard game would need hundreds of turns for.
    opts.speed = "online".into();
    opts.map_script = MapScript::Continents;
    let t0 = Instant::now();
    let mut g = Game::new_with(opts);
    let setup_s = t0.elapsed().as_secs_f64();
    write(
        &format!("{dir}/state_0.json"),
        &serde_json::to_string(&g).unwrap(),
    );
    let mut ais = AdvancedAi::fleet(&g);
    // `fog` forces last-seen memory on even for an agent that does not read
    // it, so the engine's own use of that memory is exercised too.
    let forced_fog = args.get(6).is_some_and(|a| a == "fog");
    let fog = forced_fog || ais.iter().any(Ai::uses_player_observation);
    g.set_fog_memory(fog);
    g.set_war_ledger(false);
    let play_until = |g: &mut Game, ais: &mut [AdvancedAi], turn: u32, until: Option<Instant>| {
        while g.winner.is_none() && g.turn <= g.max_turns {
            if g.current == 0 && (g.turn >= turn || until.is_some_and(|u| Instant::now() > u)) {
                break;
            }
            let pid = g.current;
            ais[pid].take_turn(g, pid);
            if g.winner.is_none() && g.current == pid {
                let _ = g.apply(pid, &Action::EndTurn);
            }
        }
    };
    let t1 = Instant::now();
    play_until(&mut g, &mut ais, t, Some(budget_end));
    let to_t_s = t1.elapsed().as_secs_f64();
    let log_at_t = g.log.len();
    let turn_at_t = g.turn;
    write(
        &format!("{dir}/state_t.json"),
        &serde_json::to_string(&g).unwrap(),
    );
    let t2 = Instant::now();
    let window_end = g.turn + k;
    play_until(&mut g, &mut ais, window_end, None);
    let window_s = t2.elapsed().as_secs_f64();
    let window: Vec<(usize, Action)> = g.log.since(log_at_t).cloned().collect();
    write(
        &format!("{dir}/actions.json"),
        &serde_json::to_string(&window).unwrap(),
    );
    let all: Vec<(usize, Action)> = g.log.iter().cloned().collect();
    write(
        &format!("{dir}/actions_all.json"),
        &serde_json::to_string(&all).unwrap(),
    );
    write(
        &format!("{dir}/final.json"),
        &serde_json::to_string(&g).unwrap(),
    );
    let meta = serde_json::json!({
        "players": players, "seed": seed, "t": turn_at_t, "end_turn": g.turn,
        "winner": g.winner, "fog": fog, "setup_s": setup_s, "to_t_s": to_t_s,
        "window_s": window_s, "actions_to_t": log_at_t, "actions_window": window.len(),
        "actions_total": all.len(), "cities": g.cities.len(), "units": g.units.len(),
        "tiles": g.map.tiles.len(), "seats": g.players.len(),
    });
    write(&format!("{dir}/meta.json"), &meta.to_string());
    println!("{meta}");
}

/// One field, addressed in the save: `key`, `key[*].sub` for a list of
/// records (players, cities, units), or `key.sub[*].leaf` for a list one
/// object down (the map's tiles).
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum FieldPath {
    Top(String),
    Each(String, String),
    NestedEach(String, String, String),
}

impl FieldPath {
    fn parse(text: &str) -> FieldPath {
        if let Some((outer, leaf)) = text.split_once("[*].") {
            match outer.split_once('.') {
                Some((key, sub)) => FieldPath::NestedEach(key.into(), sub.into(), leaf.into()),
                None => FieldPath::Each(outer.into(), leaf.into()),
            }
        } else {
            FieldPath::Top(text.into())
        }
    }

    fn show(&self) -> String {
        match self {
            FieldPath::Top(k) => k.clone(),
            FieldPath::Each(k, s) => format!("{k}[*].{s}"),
            FieldPath::NestedEach(k, s, l) => format!("{k}.{s}[*].{l}"),
        }
    }

    /// Whether `other` lies inside this field (or is it).
    fn covers(&self, other: &FieldPath) -> bool {
        match (self, other) {
            (FieldPath::Top(a), FieldPath::Top(b)) => {
                a == b
                    || b.strip_prefix(a.as_str())
                        .is_some_and(|rest| rest.starts_with('.'))
            }
            (FieldPath::Top(a), FieldPath::Each(b, _)) => a == b,
            (FieldPath::Top(a), FieldPath::NestedEach(b, _, _)) => a == b,
            (FieldPath::Each(a, s), FieldPath::Each(b, t)) => a == b && s == t,
            (FieldPath::NestedEach(a, s, l), FieldPath::NestedEach(b, t, m)) => {
                a == b && s == t && l == m
            }
            _ => false,
        }
    }
}

fn record_list(v: &Value) -> Option<&Vec<Value>> {
    match v {
        Value::Array(items) if !items.is_empty() && items.iter().all(Value::is_object) => {
            Some(items)
        }
        _ => None,
    }
}

fn union_keys(items: &[Value]) -> BTreeSet<String> {
    items
        .iter()
        .filter_map(Value::as_object)
        .flat_map(|o| o.keys().cloned())
        .collect()
}

/// Every field of the save, at the depth the report is read at.
fn enumerate(save: &Value) -> Vec<FieldPath> {
    let mut out = Vec::new();
    let Some(top) = save.as_object() else {
        return out;
    };
    for (key, value) in top {
        out.push(FieldPath::Top(key.clone()));
        if let Some(items) = record_list(value) {
            for sub in union_keys(items) {
                out.push(FieldPath::Each(key.clone(), sub));
            }
        } else if let Some(obj) = value.as_object() {
            for (sub, inner) in obj {
                if let Some(items) = record_list(inner) {
                    for leaf in union_keys(items) {
                        out.push(FieldPath::NestedEach(key.clone(), sub.clone(), leaf));
                    }
                }
            }
        }
    }
    out
}

/// The empty value of the same shape: `[]`, `{}`, `0`, `false`, `""`.
fn empty_like(v: &Value) -> Value {
    match v {
        Value::Array(_) => Value::Array(Vec::new()),
        Value::Object(_) => Value::Object(Map::new()),
        Value::Number(_) => Value::from(0),
        Value::Bool(_) => Value::Bool(false),
        Value::String(_) => Value::String(String::new()),
        Value::Null => Value::Null,
    }
}

/// Same structure, every number zero and every flag false: for fields whose
/// type will not accept an empty container (`[bool; 6]`, an RNG's words).
fn zero_like(v: &Value) -> Value {
    match v {
        Value::Array(items) => Value::Array(items.iter().map(zero_like).collect()),
        Value::Object(o) => {
            Value::Object(o.iter().map(|(k, x)| (k.clone(), zero_like(x))).collect())
        }
        Value::Number(_) => Value::from(0),
        Value::Bool(_) => Value::Bool(false),
        other => other.clone(),
    }
}

/// Blank `path` in `save` with `blank`; returns whether anything changed.
fn blank(save: &mut Value, path: &FieldPath, blank: fn(&Value) -> Value) -> bool {
    let mut changed = false;
    let mut set = |slot: &mut Value| {
        let new = blank(slot);
        if *slot != new {
            *slot = new;
            changed = true;
        }
    };
    match path {
        FieldPath::Top(k) => {
            if let Some(slot) = save.get_mut(k) {
                set(slot);
            }
        }
        FieldPath::Each(k, s) => {
            if let Some(Value::Array(items)) = save.get_mut(k) {
                for item in items {
                    if let Some(slot) = item.get_mut(s) {
                        set(slot);
                    }
                }
            }
        }
        FieldPath::NestedEach(k, s, l) => {
            if let Some(Value::Array(items)) = save.get_mut(k).and_then(|o| o.get_mut(s)) {
                for item in items {
                    if let Some(slot) = item.get_mut(l) {
                        set(slot);
                    }
                }
            }
        }
    }
    changed
}

/// The fields at which two end states differ, at report depth.
fn diff(a: &Value, b: &Value) -> Vec<FieldPath> {
    let mut out = Vec::new();
    let (Some(ao), Some(bo)) = (a.as_object(), b.as_object()) else {
        return vec![FieldPath::Top("<root>".into())];
    };
    let keys: BTreeSet<&String> = ao.keys().chain(bo.keys()).collect();
    for key in keys {
        let (x, y) = (ao.get(key), bo.get(key));
        if x == y {
            continue;
        }
        match (x, y) {
            (Some(Value::Array(xs)), Some(Value::Array(ys)))
                if xs.iter().chain(ys.iter()).all(Value::is_object)
                    && !(xs.is_empty() && ys.is_empty()) =>
            {
                let subs: BTreeSet<String> =
                    union_keys(xs).into_iter().chain(union_keys(ys)).collect();
                if xs.len() != ys.len() {
                    out.push(FieldPath::Each(key.clone(), "<count>".into()));
                }
                for sub in subs {
                    let differs = xs.len() != ys.len()
                        || xs
                            .iter()
                            .zip(ys.iter())
                            .any(|(p, q)| p.get(&sub) != q.get(&sub));
                    if differs && xs.len() == ys.len() {
                        out.push(FieldPath::Each(key.clone(), sub));
                    }
                }
            }
            (Some(Value::Object(xo)), Some(Value::Object(yo))) => {
                let subs: BTreeSet<&String> = xo.keys().chain(yo.keys()).collect();
                let mut nested = false;
                for sub in subs {
                    let (p, q) = (xo.get(sub), yo.get(sub));
                    if p == q {
                        continue;
                    }
                    if let (Some(Value::Array(ps)), Some(Value::Array(qs))) = (p, q) {
                        if ps.len() == qs.len() && ps.iter().chain(qs.iter()).all(Value::is_object)
                        {
                            let leaves: BTreeSet<String> =
                                union_keys(ps).into_iter().chain(union_keys(qs)).collect();
                            for leaf in leaves {
                                if ps
                                    .iter()
                                    .zip(qs.iter())
                                    .any(|(m, n)| m.get(&leaf) != n.get(&leaf))
                                {
                                    out.push(FieldPath::NestedEach(key.clone(), sub.clone(), leaf));
                                    nested = true;
                                }
                            }
                            continue;
                        }
                    }
                    out.push(FieldPath::Top(format!("{key}.{sub}")));
                    nested = true;
                }
                if !nested {
                    out.push(FieldPath::Top(key.clone()));
                }
            }
            _ => out.push(FieldPath::Top(key.clone())),
        }
    }
    out
}

struct Replayed {
    end: Value,
    /// The world at the first turn boundary after the save: what a field
    /// rebuilt every turn has already been rebuilt to.
    checkpoint: Option<Value>,
    /// `(round, seat, legal action count, hash of the legal set)` at the
    /// start of each seat's turn in the first and the middle round: the
    /// actions nobody took are part of what the state decides.
    legal: Vec<(u32, usize, usize, u64)>,
    refused: usize,
    first_refusal: Option<(usize, String)>,
}

enum Outcome {
    LoadError(String),
    LoadPanic,
    ReplayPanic(usize),
    Done(Replayed),
}

/// Through text, as a save is read: an `f32` widened straight into a `Value`
/// would not compare equal to the same `f32` read back from a file.
fn as_saved(g: &Game) -> Value {
    serde_json::from_str(&serde_json::to_string(g).unwrap()).unwrap()
}

fn legal_hash(g: &Game, pid: usize) -> (usize, u64) {
    use std::hash::{Hash, Hasher};
    let mut set: Vec<String> = g
        .legal_actions(pid)
        .iter()
        .map(|a| serde_json::to_string(a).unwrap())
        .collect();
    set.sort();
    let mut h = std::collections::hash_map::DefaultHasher::new();
    set.hash(&mut h);
    (set.len(), h.finish())
}

fn replay(save: Value, actions: &[(usize, Action)], fog: bool, rounds: &[u32]) -> Outcome {
    let loaded = catch_unwind(AssertUnwindSafe(|| serde_json::from_value::<Game>(save)));
    let mut g = match loaded {
        Ok(Ok(g)) => g,
        Ok(Err(e)) => return Outcome::LoadError(e.to_string().chars().take(160).collect()),
        Err(_) => return Outcome::LoadPanic,
    };
    g.set_fog_memory(fog);
    g.set_war_ledger(false);
    let start_turn = g.turn;
    let mut refused = 0usize;
    let mut first_refusal = None;
    let mut applied = 0usize;
    let mut checkpoint = None;
    let mut legal = Vec::new();
    let mut sampled: BTreeSet<(u32, usize)> = BTreeSet::new();
    let ran = catch_unwind(AssertUnwindSafe(|| {
        for (i, (pid, action)) in actions.iter().enumerate() {
            applied = i;
            let round = g.turn.saturating_sub(start_turn);
            if checkpoint.is_none() && round > 0 {
                checkpoint = Some(as_saved(&g));
            }
            if rounds.contains(&round) && g.winner.is_none() && sampled.insert((round, g.current)) {
                let (n, h) = legal_hash(&g, g.current);
                legal.push((round, g.current, n, h));
            }
            if let Err(reason) = g.apply(*pid, action) {
                refused += 1;
                if first_refusal.is_none() {
                    first_refusal = Some((i, reason.chars().take(120).collect::<String>()));
                }
            }
        }
    }));
    if ran.is_err() {
        return Outcome::ReplayPanic(applied);
    }
    match catch_unwind(AssertUnwindSafe(|| as_saved(&g))) {
        Ok(end) => Outcome::Done(Replayed {
            end,
            checkpoint,
            legal,
            refused,
            first_refusal,
        }),
        Err(_) => Outcome::ReplayPanic(actions.len()),
    }
}

fn get_path<'a>(v: &'a Value, path: &FieldPath) -> Vec<Option<&'a Value>> {
    match path {
        FieldPath::Top(k) => vec![v.get(k)],
        FieldPath::Each(k, s) => match v.get(k) {
            Some(Value::Array(items)) => items.iter().map(|i| i.get(s)).collect(),
            _ => vec![None],
        },
        FieldPath::NestedEach(k, s, l) => match v.get(k).and_then(|o| o.get(s)) {
            Some(Value::Array(items)) => items.iter().map(|i| i.get(l)).collect(),
            _ => vec![None],
        },
    }
}

fn meta_of(dir: &str) -> Value {
    serde_json::from_str::<Value>(&read(&format!("{dir}/meta.json"))).unwrap_or(Value::Null)
}

fn fog_of(dir: &str) -> bool {
    meta_of(dir)
        .get("fog")
        .and_then(Value::as_bool)
        .unwrap_or(true)
}

/// The first round, and the one halfway through the window.
fn legal_rounds(dir: &str) -> Vec<u32> {
    let meta = meta_of(dir);
    let t = meta.get("t").and_then(Value::as_u64).unwrap_or(0) as u32;
    let end = meta.get("end_turn").and_then(Value::as_u64).unwrap_or(0) as u32;
    vec![0, end.saturating_sub(t) / 2]
}

fn outcome_json(
    path: &str,
    mode: &str,
    outcome: &Outcome,
    reference: &Replayed,
    ablated: &[FieldPath],
) -> Value {
    match outcome {
        Outcome::LoadError(e) => serde_json::json!({"path": path, "mode": mode, "load_error": e}),
        Outcome::LoadPanic => serde_json::json!({"path": path, "mode": mode, "load_panic": true}),
        Outcome::ReplayPanic(at) => {
            serde_json::json!({"path": path, "mode": mode, "replay_panic_at": at})
        }
        Outcome::Done(r) => {
            let diverged: Vec<String> = diff(&reference.end, &r.end)
                .into_iter()
                .filter(|d| !ablated.iter().any(|a| a.covers(d)))
                .map(|d| d.show())
                .collect();
            let restored = ablated
                .iter()
                .all(|a| get_path(&reference.end, a) == get_path(&r.end, a));
            let restored_at_checkpoint = match (&reference.checkpoint, &r.checkpoint) {
                (Some(x), Some(y)) => ablated.iter().all(|a| get_path(x, a) == get_path(y, a)),
                _ => false,
            };
            let legal_differs = reference
                .legal
                .iter()
                .zip(r.legal.iter())
                .find(|(x, y)| x != y)
                .map(|(x, y)| serde_json::json!([x.0, x.1, x.2, y.2]))
                .or_else(|| {
                    (reference.legal.len() != r.legal.len())
                        .then(|| serde_json::json!(["count", reference.legal.len(), r.legal.len()]))
                });
            let count = diverged.len();
            serde_json::json!({
                "path": path, "mode": mode, "refused": r.refused,
                "first_refusal": r.first_refusal, "diverged_count": count,
                "diverged": diverged.into_iter().take(30).collect::<Vec<_>>(),
                "restored": restored, "restored_at_checkpoint": restored_at_checkpoint,
                "legal_differs": legal_differs,
            })
        }
    }
}

/// Which counter keys differ between two worlds, per seat: the counters map
/// holds engine tallies and agents' private notes side by side.
fn counter_keys_that_differ(a: &Value, b: &Value) -> Vec<String> {
    let (Some(xs), Some(ys)) = (
        a.get("players").and_then(Value::as_array),
        b.get("players").and_then(Value::as_array),
    ) else {
        return vec![];
    };
    let mut keys = BTreeSet::new();
    for (x, y) in xs.iter().zip(ys) {
        let (cx, cy) = (x.get("counters"), y.get("counters"));
        let (Some(cx), Some(cy)) = (cx.and_then(Value::as_object), cy.and_then(Value::as_object))
        else {
            continue;
        };
        for k in cx.keys().chain(cy.keys()) {
            if cx.get(k) != cy.get(k) {
                keys.insert(k.clone());
            }
        }
    }
    keys.into_iter().collect()
}

fn ablate(args: &[String]) {
    let dir = &args[0];
    let deadline_s: f64 = args.get(1).map(|s| s.parse().unwrap()).unwrap_or(1e9);
    let started = Instant::now();
    let fog = fog_of(dir);
    let rounds = legal_rounds(dir);
    let save: Value = serde_json::from_str(&read(&format!("{dir}/state_t.json"))).unwrap();
    let actions: Vec<(usize, Action)> =
        serde_json::from_str(&read(&format!("{dir}/actions.json"))).unwrap();
    let recorded: Value = serde_json::from_str(&read(&format!("{dir}/final.json"))).unwrap();

    // Does the save, untouched, replay to the game that was played? What it
    // does not reproduce was written by something other than an action.
    let t = Instant::now();
    let reference = match replay(save.clone(), &actions, fog, &rounds) {
        Outcome::Done(r) => {
            let missing: Vec<String> = diff(&recorded, &r.end)
                .into_iter()
                .map(|d| d.show())
                .collect();
            println!(
                "{}",
                serde_json::json!({"baseline": true, "refused": r.refused,
                    "first_refusal": r.first_refusal, "differs_from_recorded": missing,
                    "counter_keys": counter_keys_that_differ(&recorded, &r.end),
                    "legal_samples": r.legal.len(),
                    "replay_s": t.elapsed().as_secs_f64()})
            );
            r
        }
        _ => {
            println!("{}", serde_json::json!({"baseline": true, "failed": true}));
            return;
        }
    };

    // From the very start: does setup plus the whole log reproduce it too?
    let t = Instant::now();
    let all: Vec<(usize, Action)> =
        serde_json::from_str(&read(&format!("{dir}/actions_all.json"))).unwrap();
    let zero: Value = serde_json::from_str(&read(&format!("{dir}/state_0.json"))).unwrap();
    if let Outcome::Done(r) = replay(zero, &all, fog, &[]) {
        let missing: Vec<String> = diff(&recorded, &r.end)
            .into_iter()
            .map(|d| d.show())
            .collect();
        println!(
            "{}",
            serde_json::json!({"from_zero": true, "refused": r.refused,
                "first_refusal": r.first_refusal, "differs_from_recorded": missing,
                "counter_keys": counter_keys_that_differ(&recorded, &r.end),
                "replay_s": t.elapsed().as_secs_f64()})
        );
    } else {
        println!("{}", serde_json::json!({"from_zero": true, "failed": true}));
    }

    for path in enumerate(&save) {
        if started.elapsed().as_secs_f64() > deadline_s {
            println!(
                "{}",
                serde_json::json!({"deadline": true, "at": path.show()})
            );
            break;
        }
        let shown = path.show();
        let mut ablated = save.clone();
        if !blank(&mut ablated, &path, empty_like) {
            println!("{}", serde_json::json!({"path": shown, "trivial": true}));
            continue;
        }
        let t = Instant::now();
        let mut outcome = replay(ablated, &actions, fog, &rounds);
        let mut mode = "empty";
        if matches!(outcome, Outcome::LoadError(_) | Outcome::LoadPanic) {
            let mut zeroed = save.clone();
            if blank(&mut zeroed, &path, zero_like) {
                outcome = replay(zeroed, &actions, fog, &rounds);
                mode = "zero";
            }
        }
        let mut row = outcome_json(
            &shown,
            mode,
            &outcome,
            &reference,
            std::slice::from_ref(&path),
        );
        row["s"] = Value::from((t.elapsed().as_secs_f64() * 1000.0).round() / 1000.0);
        println!("{row}");
    }
}

/// `strip <dir> <paths-file>`: each line is `path` or `path<TAB>zero`.
fn strip(args: &[String]) {
    let dir = &args[0];
    let fog = fog_of(dir);
    let rounds = legal_rounds(dir);
    let listed: Vec<(FieldPath, bool)> = read(&args[1])
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .map(|l| match l.split_once('\t') {
            Some((p, mode)) => (FieldPath::parse(p), mode == "zero"),
            None => (FieldPath::parse(l), false),
        })
        .collect();
    let paths: Vec<FieldPath> = listed.iter().map(|(p, _)| p.clone()).collect();
    let save: Value = serde_json::from_str(&read(&format!("{dir}/state_t.json"))).unwrap();
    let actions: Vec<(usize, Action)> =
        serde_json::from_str(&read(&format!("{dir}/actions.json"))).unwrap();
    let reference = match replay(save.clone(), &actions, fog, &rounds) {
        Outcome::Done(r) => r,
        _ => {
            println!(
                "{}",
                serde_json::json!({"strip": dir, "baseline_failed": true})
            );
            return;
        }
    };
    let mut stripped = save.clone();
    let mut blanked = 0usize;
    for (p, zero) in &listed {
        let how: fn(&Value) -> Value = if *zero { zero_like } else { empty_like };
        if blank(&mut stripped, p, how) {
            blanked += 1;
        }
    }
    let before = serde_json::to_string(&save).unwrap().len();
    let after = serde_json::to_string(&stripped).unwrap().len();
    write(
        &format!("{dir}/stripped.json"),
        &serde_json::to_string(&stripped).unwrap(),
    );
    let outcome = replay(stripped, &actions, fog, &rounds);
    let mut row = outcome_json(
        &format!("<strip {} paths>", paths.len()),
        "mixed",
        &outcome,
        &reference,
        &paths,
    );
    row["strip"] = Value::from(dir.as_str());
    row["blanked"] = Value::from(blanked);
    row["bytes_before"] = Value::from(before);
    row["bytes_after"] = Value::from(after);
    println!("{row}");
}

/// What a position costs to hold: clone and serialize a loaded save. Run on
/// the full save and on the stripped one, it prices everything the
/// transition never reads.
fn bench(args: &[String]) {
    let path = &args[0];
    let n: usize = args.get(1).map(|s| s.parse().unwrap()).unwrap_or(2000);
    let g: Game = serde_json::from_str(&read(path)).unwrap();
    let mut keep = 0usize;
    let mut samples = Vec::with_capacity(9);
    for _ in 0..9 {
        let t = Instant::now();
        for _ in 0..n {
            let c = g.clone();
            keep = keep.wrapping_add(c.units.len());
        }
        samples.push(t.elapsed().as_secs_f64() / n as f64);
    }
    samples.sort_by_shared(f64::total_cmp);
    let clone_us = samples[4] * 1e6;
    // A clone is shallow: seats, units and the map are shared until written.
    // What a search branch really pays is the first write to each, which
    // copies the whole seat, the unit table or the tile vector, every field
    // in it read by the game or not.
    let median = |f: &dyn Fn()| {
        let mut xs: Vec<f64> = (0..9)
            .map(|_| {
                let t = Instant::now();
                for _ in 0..n {
                    f();
                }
                t.elapsed().as_secs_f64() / n as f64 * 1e6
            })
            .collect();
        xs.sort_by_shared(f64::total_cmp);
        xs[4] - clone_us
    };
    let seats = g.players.len();
    let seat_cow_us = median(&|| {
        let mut c = g.clone();
        for i in 0..seats {
            c.players[i].gold += 0.0;
        }
    }) / seats.max(1) as f64;
    let first_tile = g.map.tiles.values().next().map(|t| t.pos);
    let map_cow_us = median(&|| {
        let mut c = g.clone();
        if let Some(pos) = first_tile {
            if let Some(t) = c.map.tiles.get_mut(&pos) {
                t.road = t.road.wrapping_add(0);
            }
        }
    });
    let first_unit = g.units.keys().next().copied();
    let unit_cow_us = median(&|| {
        let mut c = g.clone();
        if let Some(id) = first_unit {
            if let Some(u) = c.units.get_mut(&id) {
                u.hp += 0;
            }
        }
    });
    let t = Instant::now();
    let reps = 20;
    let mut bytes = 0usize;
    for _ in 0..reps {
        bytes = serde_json::to_string(&g).unwrap().len();
    }
    let serialize_ms = t.elapsed().as_secs_f64() / reps as f64 * 1e3;
    println!(
        "{}",
        serde_json::json!({"bench": path, "clone_us": clone_us, "seat_cow_us": seat_cow_us,
            "map_cow_us": map_cow_us, "unit_cow_us": unit_cow_us, "seats": seats,
            "serialize_ms": serialize_ms, "json_bytes": bytes, "keep": keep})
    );
}

/// `agent <save.json> <turns> <out.json> [fog]`: fresh stock agents play on
/// from a save. Played once from the full save and once from a stripped one,
/// the difference is what the agents themselves read among the fields the
/// rules do not.
fn agent(args: &[String]) {
    let mut g: Game = serde_json::from_str(&read(&args[0])).unwrap();
    let turns: u32 = args[1].parse().unwrap();
    let fog = args.get(3).is_some_and(|a| a == "fog");
    let mut ais = AdvancedAi::fleet(&g);
    g.set_fog_memory(fog || ais.iter().any(Ai::uses_player_observation));
    g.set_war_ledger(false);
    let end = g.turn + turns;
    let t = Instant::now();
    while g.winner.is_none() && g.turn <= g.max_turns && !(g.turn >= end && g.current == 0) {
        let pid = g.current;
        ais[pid].take_turn(&mut g, pid);
        if g.winner.is_none() && g.current == pid {
            let _ = g.apply(pid, &Action::EndTurn);
        }
    }
    write(&args[2], &serde_json::to_string(&g).unwrap());
    println!(
        "{}",
        serde_json::json!({"agent": &args[0], "turns": turns, "secs": t.elapsed().as_secs_f64(),
            "log": g.log.len(), "turn": g.turn})
    );
}

/// `blank <in.json> <paths-file> <out.json>`: a save with the listed fields
/// blanked (`path` or `path<TAB>zero` per line), nothing replayed.
fn blank_file(args: &[String]) {
    let mut save: Value = serde_json::from_str(&read(&args[0])).unwrap();
    let mut blanked = 0usize;
    for line in read(&args[1])
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
    {
        let (p, zero) = match line.split_once('\t') {
            Some((p, mode)) => (p, mode == "zero"),
            None => (line, false),
        };
        let how: fn(&Value) -> Value = if zero { zero_like } else { empty_like };
        if blank(&mut save, &FieldPath::parse(p), how) {
            blanked += 1;
        }
    }
    write(&args[2], &serde_json::to_string(&save).unwrap());
    println!(
        "{}",
        serde_json::json!({"blank": &args[0], "blanked": blanked})
    );
}

/// `diff <a.json> <b.json>`: the fields at which two saves differ.
fn diff_files(args: &[String]) {
    let a: Value = serde_json::from_str(&read(&args[0])).unwrap();
    let b: Value = serde_json::from_str(&read(&args[1])).unwrap();
    let paths: Vec<String> = diff(&a, &b).into_iter().map(|d| d.show()).collect();
    println!(
        "{}",
        serde_json::json!({"a": &args[0], "b": &args[1], "count": paths.len(), "paths": paths})
    );
}
