use super::*;
use crate::server::{stock_opening_params, Session};

fn reading(turn: u32, need: &[bool], avoid: &[bool], want: &[f64], dont_want: &[f64]) -> Reading {
    Reading {
        turn,
        need: need.to_vec(),
        avoid: avoid.to_vec(),
        want: want.iter().map(|value| Some(*value)).collect(),
        dont_want: dont_want.iter().map(|value| Some(*value)).collect(),
    }
}

fn objective(asked: Value) -> Objective {
    Objective::parse(&asked).expect("an objective")
}

#[test]
fn nothing_named_is_win_and_do_not_lose() {
    let default = Objective::parse(&json!({})).unwrap();
    assert_eq!(default, Objective::win_and_do_not_lose());
    assert_eq!(
        default.json(),
        json!({
            "need": [{"seat": 0, "key": "won", "test": "==", "value": 1.0, "when": "end"}],
            "avoid": [{"seat": 0, "key": "lost", "test": "==", "value": 1.0, "when": "always"}],
            "want": [],
            "dont_want": [],
        })
    );
    // Named lists are taken as named: the defaults are not added to them.
    let wanted = objective(json!({"want": [{"key": "score"}]}));
    assert!(wanted.need.is_empty() && wanted.avoid.is_empty());
    assert_eq!(wanted.want[0].weight, 1.0);
}

#[test]
fn a_term_that_cannot_be_held_is_refused() {
    let refused = |asked: Value| Objective::parse(&asked).unwrap_err();
    assert!(
        refused(json!({"need": [{"key": "cities", "test": "changes", "value": 1}]}))
            .contains("test")
    );
    assert!(refused(json!({"need": [{"key": "cities", "test": ">="}]})).contains("number"));
    assert!(
        refused(json!({"need": [{"key": "cities", "value": 3, "when": "by"}]})).contains("turn")
    );
    assert!(
        refused(json!({"need": [{"key": "cities", "value": 3, "when": "soon"}]})).contains("when")
    );
    assert!(refused(json!({"want": [{"key": "score", "weight": 0}]})).contains("weight"));
    assert!(refused(json!({"want": [{"weight": 1}]})).contains("key"));
    assert!(refused(json!({"want": []})).contains("something"));
    let many: Vec<Value> = (0..9).map(|_| json!({"key": "score"})).collect();
    assert!(refused(json!({"want": many})).contains("at most"));
}

/// When a Need holds and a Must Avoid is kept, for each when.
#[test]
fn each_when_is_judged_over_the_turns_played() {
    let lists = |when: Value| {
        let mut need = json!({"key": "cities", "test": ">=", "value": 3});
        let mut avoid = json!({"key": "lost", "test": "==", "value": 1});
        for term in [&mut need, &mut avoid] {
            for (key, value) in when.as_object().unwrap() {
                term[key] = value.clone();
            }
        }
        objective(json!({"need": [need], "avoid": [avoid]}))
    };
    // Held on turns 3 and 4 of 2..5; the Must Avoid broken on turn 3 only.
    let series = |held: [bool; 4], broken: [bool; 4]| -> Vec<Reading> {
        (0..4)
            .map(|at| reading(2 + at as u32, &[held[at]], &[broken[at]], &[], &[]))
            .collect()
    };
    let seen = series([false, true, true, false], [false, true, false, false]);
    let judged = |when: Value| {
        let verdict = lists(when).judge(&seen);
        (verdict.need[0], verdict.avoid[0])
    };
    assert_eq!(judged(json!({"when": "end"})), (false, true));
    assert_eq!(judged(json!({"when": "always"})), (false, false));
    assert_eq!(judged(json!({"when": "ever"})), (true, false));
    assert_eq!(judged(json!({"when": "by", "turn": 2})), (false, true));
    assert_eq!(judged(json!({"when": "by", "turn": 3})), (true, false));
    // A line that holds on every turn and breaks nothing meets both.
    let kept = lists(json!({"when": "always"})).judge(&series([true; 4], [false; 4]));
    assert!(kept.feasible());
    assert_eq!((kept.broken, kept.killed), (0, None));
    // Nothing played meets no Need, and breaks no Must Avoid.
    let none = lists(json!({"when": "ever"})).judge(&[]);
    assert_eq!((none.need[0], none.avoid[0], none.broken), (false, true, 1));
}

/// Only a Must Avoid that cannot come right again ends a line; a Need never
/// does, and nor does a soft number.
#[test]
fn only_a_must_avoid_broken_for_good_ends_a_line() {
    let at_end =
        objective(json!({"avoid": [{"key": "lost", "value": 1, "test": "==", "when": "end"}]}));
    let always =
        objective(json!({"avoid": [{"key": "lost", "value": 1, "test": "==", "when": "always"}]}));
    let by = objective(
        json!({"avoid": [{"key": "lost", "value": 1, "test": "==", "when": "by", "turn": 4}]}),
    );
    let need = objective(json!({"need": [{"key": "cities", "value": 9, "when": "always"}]}));
    let broken = |turn| reading(turn, &[false], &[true], &[], &[]);
    assert_eq!(at_end.kills(&broken(3)), None);
    assert_eq!(always.kills(&broken(3)), Some(0));
    assert_eq!(by.kills(&broken(4)), Some(0));
    assert_eq!(by.kills(&broken(5)), None);
    assert_eq!(need.kills(&broken(3)), None);
    let series = [reading(2, &[], &[false], &[], &[]), broken(3), broken(4)];
    assert_eq!(always.judge(&series).killed, Some(3));
}

/// A Want is percent over the seated Civ AI's line alone, so a weight means
/// the same whatever the number is; a Don't Want counts the other way.
#[test]
fn wants_are_counted_as_percent_over_the_seated_civ_ai_alone() {
    let lists = objective(json!({
        "want": [{"key": "score", "weight": 1}, {"key": "cities", "weight": 2}],
        "dont_want": [{"key": "war_weariness", "weight": 0.5}],
    }));
    let verdict = |score, cities, weariness| {
        lists.judge(&[reading(9, &[], &[], &[score, cities], &[weariness])])
    };
    let base = verdict(200.0, 4.0, 10.0);
    let line = verdict(220.0, 5.0, 20.0);
    assert_eq!(lists.over(&line, &base), vec![10.0, 25.0, -100.0]);
    assert_eq!(lists.gain(&line, &base), 10.0 + 50.0 - 50.0);
    // Against nothing, a number is counted as itself.
    let from_zero = verdict(0.0, 0.0, 0.0);
    assert_eq!(lists.over(&verdict(3.0, 0.0, 0.0), &from_zero)[0], 300.0);
}

fn line(plan: Plan, broken: usize, gain: f64, over: Vec<f64>) -> Line {
    Line {
        key: key(&plan),
        plan,
        last: 9,
        series: Vec::new(),
        verdict: Verdict {
            broken,
            ..Verdict::default()
        },
        gain,
        over,
    }
}

#[test]
fn the_ranking_is_needs_then_fewest_broken_then_gain_then_fewer_handovers() {
    let one = |name: &str| vec![(1, name.to_string())];
    let two = vec![(1, "a".to_string()), (5, "b".to_string())];
    let feasible = line(one("a"), 0, -50.0, vec![]);
    let broken_once = line(one("b"), 1, 900.0, vec![]);
    let broken_twice = line(one("c"), 2, 900.0, vec![]);
    assert!(Search::better(&feasible, &broken_once));
    assert!(Search::better(&broken_once, &broken_twice));
    assert!(Search::better(
        &line(one("a"), 0, 5.0, vec![]),
        &line(one("b"), 0, 4.0, vec![])
    ));
    assert!(Search::better(
        &line(one("a"), 0, 5.0, vec![]),
        &line(two, 0, 5.0, vec![])
    ));
    // The front: nothing beats a line on every term at once.
    let mixed = line(one("a"), 0, 0.0, vec![10.0, -5.0]);
    let other = line(one("b"), 0, 0.0, vec![-5.0, 10.0]);
    let beaten = line(one("c"), 0, 0.0, vec![-6.0, 9.0]);
    assert!(!Search::dominates(&mixed, &other) && !Search::dominates(&other, &mixed));
    assert!(Search::dominates(&other, &beaten));
    assert!(!Search::dominates(&other, &other));
}

/// A toy world: the line's number is what each turn adds, by who held the
/// seat on the turn before. `a` adds 2 a turn up to turn 5 and nothing after;
/// `b` adds 1 a turn throughout. Either alone ends on 8 by turn 9; `a` up to
/// turn 5 and `b` after it ends on 12, which only a handover finds.
fn toy(plan: &Plan, from: u32, end: u32) -> Vec<Reading> {
    let mut total = 0.0;
    (from + 1..=end)
        .map(|turn| {
            total += match who(plan, turn - 1) {
                "a" if turn <= 5 => 2.0,
                "a" => 0.0,
                _ => 1.0,
            };
            reading(turn, &[], &[], &[total], &[])
        })
        .collect()
}

fn toy_search(asked: Value) -> Search {
    Search::new(Spec::parse(&asked).expect("a spec"))
}

fn climb(search: &mut Search) -> usize {
    let (from, end) = (search.spec().from, search.spec().end);
    let mut played = 0;
    while let Some(plan) = search.next(None, from) {
        let series = toy(&plan, from, end);
        search.took(plan, series, end);
        played += 1;
        assert!(played < 2000, "the climb did not stop");
    }
    assert!(search.done());
    played
}

#[test]
fn the_climb_finds_the_handover_neither_civ_ai_alone_has() {
    let mut search = toy_search(json!({
        "objective": {"want": [{"key": "score"}]},
        "from": 1, "end": 9, "letters": ["a", "b"], "seated": "b", "forks": 1,
    }));
    // Every Civ AI alone comes first, the seated one among them.
    let first: Vec<Plan> = (0..2).map(|_| search.next(None, 1).unwrap()).collect();
    assert_eq!(
        first,
        vec![vec![(1, "a".to_string())], vec![(1, "b".to_string())]]
    );
    for plan in first {
        let series = toy(&plan, 1, 9);
        search.took(plan, series, 9);
    }
    assert_eq!(search.baseline().unwrap().plan, vec![(1, "b".to_string())]);
    climb(&mut search);
    let best = search.best().unwrap();
    assert_eq!(best.plan, vec![(1, "a".to_string()), (5, "b".to_string())]);
    assert_eq!(best.verdict.want, vec![Some(12.0)]);
    assert_eq!(best.gain, 50.0, "12 against b's 8 alone");
    let report = search.report();
    assert_eq!(report["best"]["plan"], json!([[1, "a"], [5, "b"]]));
    assert_eq!(report["best"]["chart"][3], json!([5, 8.0]));
    assert_eq!(report["front"][0]["key"], json!("1:a 5:b"));
    assert_eq!(report["alone"].as_array().unwrap().len(), 2);
}

/// What is Needed comes before what is Wanted: a line that breaks a Need is
/// passed by one that keeps it, however much more it has.
#[test]
fn a_need_outranks_any_amount_wanted() {
    let mut search = toy_search(json!({
        "objective": {
            "need": [{"key": "score", "test": "<=", "value": 8, "when": "end"}],
            "want": [{"key": "score"}],
        },
        "from": 1, "end": 9, "letters": ["a", "b"], "seated": "b",
    }));
    let (from, end) = (1, 9);
    let need = |series: Vec<Reading>| {
        series
            .into_iter()
            .map(|mut reading| {
                reading.need = vec![reading.want[0].unwrap() <= 8.0];
                reading
            })
            .collect::<Vec<_>>()
    };
    while let Some(plan) = search.next(None, from) {
        let series = need(toy(&plan, from, end));
        search.took(plan, series, end);
    }
    let best = search.best().unwrap();
    assert!(best.verdict.feasible());
    assert_eq!(best.verdict.want, vec![Some(8.0)]);
    assert!(search.lines().iter().any(|line| !line.verdict.feasible()));
}

#[test]
fn an_offered_line_is_played_and_judged_like_any_other() {
    let mut search = toy_search(json!({
        "objective": {"want": [{"key": "score"}]},
        "from": 1, "end": 9, "letters": ["a", "b"], "seated": "b",
    }));
    let handover = vec![(1, "a".to_string()), (5, "b".to_string())];
    assert_eq!(search.offer(handover.clone()), Ok(true));
    assert_eq!(
        search.offer(handover.clone()),
        Ok(false),
        "offered twice, queued once"
    );
    assert!(search
        .offer(vec![(1, "c".to_string())])
        .unwrap_err()
        .contains("does not seat"));
    assert!(search
        .offer(vec![(1, "a".to_string()), (9, "b".to_string())])
        .unwrap_err()
        .contains("past"));
    assert!(search.offer(vec![]).is_err());
    let played: Vec<Plan> = std::iter::from_fn(|| search.next(None, 1))
        .take(3)
        .collect();
    assert!(played.contains(&handover));
    // A line a copy could not play goes back once, and then is let go.
    search.failed(handover.clone());
    assert_eq!(search.next(None, 1), Some(handover.clone()));
    search.failed(handover);
}

/// A free copy takes the line it already holds the start of.
#[test]
fn a_copy_takes_the_line_it_shares_the_most_turns_of() {
    let mut search = toy_search(json!({
        "objective": {"want": [{"key": "score"}]},
        "from": 1, "end": 9, "letters": ["a", "b", "c"], "seated": "a", "forks": 2,
    }));
    let holding = vec![(1, "c".to_string())];
    assert_eq!(
        search.next(Some(&holding), 9),
        Some(vec![(1, "c".to_string())])
    );
    let parts = search.parts(
        Some(&vec![(1, "a".to_string()), (6, "b".to_string())]),
        9,
        &vec![(1, "a".to_string()), (7, "b".to_string())],
    );
    assert_eq!(parts, 6);
}

#[test]
fn a_spec_seats_its_seated_civ_ai_and_keeps_to_its_bounds() {
    let spec =
        Spec::parse(&json!({"from": 10, "turns": 30, "letters": ["a"], "seated": "b"})).unwrap();
    assert_eq!(
        (spec.from, spec.end, spec.letters.clone()),
        (10, 40, vec!["b".to_string(), "a".to_string()])
    );
    assert_eq!(spec.objective, Objective::win_and_do_not_lose());
    assert!(Spec::parse(&json!({"from": 10, "end": 10, "letters": ["a"]})).is_err());
    assert!(Spec::parse(&json!({"from": 10, "turns": 9000, "letters": ["a"]})).is_err());
    assert!(Spec::parse(&json!({"from": 10, "turns": 5})).is_err());
    assert_eq!(strides(30), vec![8, 4, 2, 1]);
    assert_eq!(strides(100), vec![25, 13, 7, 4, 3]);
}

#[test]
fn the_route_runs_a_search_for_a_page() {
    let mut held = None;
    assert!(route(&mut held, &json!({"op": "next"}))["error"].is_string());
    let begun = route(
        &mut held,
        &json!({"op": "begin", "objective": {"want": [{"key": "score"}]},
        "from": 1, "end": 9, "letters": ["a", "b"], "seated": "b"}),
    );
    assert_eq!(begun["lines"], json!(0));
    loop {
        let next = route(&mut held, &json!({"op": "next", "at": 1}));
        let Some(plan) = next["plan"].as_array().filter(|_| !next["plan"].is_null()) else {
            assert_eq!(next["done"], json!(true));
            break;
        };
        let plan: Plan = serde_json::from_value(json!(plan)).unwrap();
        let series = toy(&plan, 1, 9);
        route(
            &mut held,
            &json!({"op": "took", "plan": plan, "series": series, "last": 9}),
        );
    }
    let report = route(&mut held, &json!({"op": "report"}));
    assert_eq!(report["best"]["key"], json!("1:a 5:b"));
    let offered = route(
        &mut held,
        &json!({"op": "offer", "plans": [[[1, "b"], [3, "a"]], [[1, "z"]]]}),
    );
    assert_eq!(offered["offers"][0], json!(true));
    assert!(offered["offers"][1]
        .as_str()
        .unwrap()
        .contains("does not seat"));
    assert_eq!(
        route(&mut held, &json!({"op": "end"}))["ended"],
        json!(true)
    );
    assert!(held.is_none());
}

/// A played game, from the site's opening world, with a seat for the debugger.
fn played() -> Session {
    let mut params = stock_opening_params(20_261_123);
    params.spectate = false;
    let mut session = Session::new(params);
    session.debug(&json!({"op": "status", "strategy": "jack"}));
    session
}

/// The engine reads every term on every turn a `play` plays, off the seat's
/// row of the standings or, for a key that row lacks, off the omniscient
/// observation; and a key it has no number for is refused before anything
/// is played.
#[test]
fn a_play_reads_the_terms_of_the_objective_it_was_given() {
    let mut session = played();
    let refused = session
        .debug(&json!({"op": "objective", "objective": {"want": [{"key": "no such thing"}]}}));
    assert!(refused["error"]
        .as_str()
        .unwrap()
        .contains("no number called"));
    let set = session.debug(&json!({"op": "objective", "lean": true, "objective": {
        "need": [{"key": "won", "test": "==", "value": 1}],
        "avoid": [{"key": "lost", "test": "==", "value": 1, "when": "always"}],
        "want": [{"key": "score"}, {"key": "yields.science"}, {"key": "techs"}],
        "dont_want": [{"key": "cities", "seat": 1}],
    }}));
    assert_eq!(set["error"], Value::Null);
    let answer = session.debug(&json!({"op": "play", "turns": 3, "lean": true}));
    let terms: Vec<Reading> = serde_json::from_value(answer["terms"].clone()).unwrap();
    assert_eq!(terms.len(), 3);
    assert_eq!(answer["killed"], Value::Null);
    let last = &terms[2];
    assert_eq!(last.turn, session.game.turn);
    assert_eq!(
        (last.need.clone(), last.avoid.clone()),
        (vec![false], vec![false])
    );
    assert_eq!(last.want[0], Some(session.game.score(0) as f64));
    assert!(
        last.want.iter().chain(&last.dont_want).all(Option::is_some),
        "{last:?}"
    );
    // Without an objective a play answers as it always has.
    session.debug(&json!({"op": "objective", "objective": null}));
    let plain = session.debug(&json!({"op": "play", "turns": 1, "lean": true}));
    assert!(plain.get("terms").is_none() && plain["readings"].is_array());
}

/// A Must Avoid broken for good ends the play on the turn it is broken.
#[test]
fn a_must_avoid_broken_for_good_ends_the_play_there() {
    let mut session = played();
    session.debug(&json!({"op": "objective", "objective": {
        "avoid": [{"key": "cities", "test": ">=", "value": 1, "when": "always"}],
    }}));
    let answer = session.debug(&json!({"op": "play", "turns": 20, "lean": true}));
    let terms: Vec<Reading> = serde_json::from_value(answer["terms"].clone()).unwrap();
    let ended = answer["killed"]["turn"]
        .as_u64()
        .expect("the first city ends it") as u32;
    assert_eq!(terms.last().unwrap().turn, ended);
    assert!(terms.len() < 20);
    assert_eq!(session.game.turn, ended);
}

#[test]
fn a_civ_ai_is_named_with_a_victory_target_and_genes() {
    let (_, written) = crate::strategies::build_named("JackOfAllTrades").unwrap();
    assert_eq!(written, "jack");
    let (_, written) =
        crate::strategies::build_named("jack@Science+no-city-limit+no-city-limit").unwrap();
    assert_eq!(written, "jack@science+no-city-limit");
    let refused = |name: &str| {
        crate::strategies::build_named(name)
            .err()
            .unwrap_or_default()
    };
    assert!(refused("jack+no such gene").contains("gene"));
    assert!(!refused("jack@nowhere").is_empty());
    assert!(refused("wheels@science").contains("scripted"));
    let mut session = played();
    let seated = session.debug(&json!({"op": "status", "strategy": "jack@science+no-city-limit"}));
    assert_eq!(seated["error"], Value::Null);
    assert_eq!(
        session.seated_strategy_name(0),
        Some("jack@science+no-city-limit")
    );
}

/// The native runner: a search on this process's own copies of the engine,
/// a thread each, and the best line played again on another world.
#[test]
fn the_native_runner_searches_on_threads_and_plays_the_line_again_elsewhere() {
    let options = RunOptions {
        seed: 20_261_123,
        spec: json!({
            "objective": {"want": [{"key": "score"}]},
            "from": 2, "turns": 3, "letters": ["jack", "stock"], "seated": "jack",
        }),
        cores: 2,
        offers: vec![vec![(2, "stock".to_string()), (4, "jack".to_string())]],
        robust: vec![20_261_124],
        fog_honest: None,
    };
    let report = run(&options).unwrap();
    assert!(report["lines"].as_u64().unwrap() >= 3, "{report}");
    assert_eq!(report["offered"], json!(1));
    assert_eq!(report["queued"], json!(0));
    assert!(report["best"]["gain"].as_f64().unwrap() >= 0.0);
    let robust = &report["robust"];
    assert_eq!(robust["played"], json!(1), "{robust}");
    assert_eq!(robust["plan"], report["best"]["plan"]);
    // On one copy the climb goes the same way every time, and the same
    // search on the same world finds the same line. On several, which line
    // comes back first decides what the climb tries next, so the lines tried
    // may differ from run to run.
    let one = RunOptions {
        cores: 1,
        robust: Vec::new(),
        ..options
    };
    let (first, again) = (run(&one).unwrap(), run(&one).unwrap());
    assert_eq!(first["best"]["key"], again["best"]["key"]);
    assert_eq!(first["lines"], again["lines"]);
}
