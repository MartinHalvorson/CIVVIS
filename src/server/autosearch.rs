//! Autosearch: which Civ AIs should hold the seat, and when, to meet four
//! lists.
//!
//! Operator 2026-11-23, pasting a review of Autosearch (#5686): "don't build
//! a new system. Yesterday's Autosearch is already this idea with a
//! one-number objective. Generalize its objective into your four lists, move
//! the search core from JS into Rust, and use the model outside the
//! simulation loop, never inside it."
//!
//! The four lists are what the seat **Needs**, what it **Must Avoid**, what
//! it **Wants** and what it **Doesn't Want** ([`Objective`]). A Need or a
//! Must Avoid is a breakpoint condition (`debugger::Condition`: a seat, a key
//! of what the engine says about that seat, a test and a line) with a *when*:
//! on the last turn of the line, on every turn, on some turn, or on some turn
//! by a given one. A Want or a Don't Want is a key with a weight. Every turn
//! a line plays is read for each term ([`Objective::read`], the `terms` a
//! `play` answers with).
//!
//! The climb ([`Search`]) was `debugSearch*` in `app_debugger.js`. It begins
//! with every Civ AI holding the seat alone and climbs from the best line it
//! has: each other Civ AI is handed a stretch of it, at a spread of turns,
//! at wide strides first and finer ones after. What is new:
//!
//! - **The ranking** ([`Search::better`]). A line that meets every Need and
//!   breaks no Must Avoid comes first; then the one that fails fewest of
//!   them; then the most of what is Wanted less what is not, each counted as
//!   percent over the line the seated Civ AI plays alone, so that a weight
//!   means the same for Score as for Faith a turn; then fewer handovers.
//! - **Cutting a line only on what cannot be undone.** A Must Avoid held
//!   always, ever or by a turn ends the line on the turn it is broken, as a
//!   breakpoint ends a run ([`Objective::kills`]). Nothing is cut on a soft
//!   number: `docs/SUPERHUMAN.md` measured that "any within-review pruning
//!   discards real signal".
//! - **A front, not one best.** Every line nothing beats on every term at
//!   once is kept ([`Search::front`]), and the climb grows from them as well
//!   as from the best, which takes it past the single handover a climb from
//!   one line stops at.
//! - **A wider alphabet.** A Civ AI the seat is handed to is a roster name,
//!   or one aimed at a victory with gene tags on (`jack@science+no-city-limit`,
//!   `strategies::build_named`): a gene is a switch an agent reads as it
//!   plays.
//!
//! The search plays nothing. Whatever holds the engines plays the lines it
//! hands out and gives back what each read: the page's copies of the engine
//! (`POST /autosearch`, [`route`], which the page pumps), or this process's
//! own sessions on threads ([`run`], `civvis autosearch`), which is what lets
//! a search run without a browser, over many seeds, on the fleet.
//!
//! A language model belongs outside this loop and never in it. A turn costs
//! tens of milliseconds and a call seconds, and a model does not answer the
//! same way twice, which would break the exact walk back a line rests on.
//! From outside it can put lines in to be played and judged like any other
//! ([`Search::offer`]), turn plain words into an [`Objective`] (which the
//! world checks before a line is played, `Objective::check`), and, when no
//! line meets a Need, name the behaviour that is missing so that a gene can
//! be written for it.
//!
//! Two limits. A line found on one world is a fact about that world: the
//! runner's robust mode plays it again on other seeds and says how often it
//! still meets the Needs (`docs/GENOME.md`: "the only defence is replication
//! on disjoint maps"). And what a term reads is the omniscient observation:
//! the searcher looking at the world, which is right for exploring and says
//! nothing about what a fog-honest player could have known.

use std::collections::{BTreeMap, HashMap, HashSet, VecDeque};
use std::sync::{Condvar, Mutex};

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use super::debugger::{as_number, Test};
use super::Session;
use crate::game::Game;

/// The most terms one of the four lists holds.
const MOST_TERMS: usize = 8;
/// The most Civ AIs a search hands the seat to.
const MOST_LETTERS: usize = 64;
/// The longest a search plays ahead.
const MOST_TURNS: u32 = 400;
/// How many lines of the front, besides the best, the climb grows from.
const FRONT_SEEDS: usize = 3;
/// Lines of the front a report lists.
const FRONT_SAID: usize = 8;
/// Turns a copy of the engine plays a request in the native runner.
const CHUNK: u32 = 10;

/// When a Need must hold, or a Must Avoid must not.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum When {
    /// On the last turn the line is played to.
    End,
    /// On every turn of the line. A Must Avoid broken on any turn ends the
    /// line there.
    Always,
    /// On some turn of the line. For a Must Avoid, which must not hold on
    /// any turn, the same as `Always`.
    Ever,
    /// On some turn up to this one (a Need); on none up to it (a Must Avoid,
    /// which ends the line when broken).
    By(u32),
}

impl When {
    fn parse(asked: &Value) -> Result<When, String> {
        match asked["when"].as_str().unwrap_or("end") {
            "end" => Ok(When::End),
            "always" => Ok(When::Always),
            "ever" => Ok(When::Ever),
            "by" => asked["turn"]
                .as_u64()
                .and_then(|turn| u32::try_from(turn).ok())
                .filter(|turn| *turn > 0)
                .map(When::By)
                .ok_or_else(|| "a term held by a turn names the turn".to_string()),
            other => Err(format!(
                "a term's when is end, always, ever or by, not {other:?}"
            )),
        }
    }

    fn word(self) -> &'static str {
        match self {
            When::End => "end",
            When::Always => "always",
            When::Ever => "ever",
            When::By(_) => "by",
        }
    }
}

/// A Need or a Must Avoid: a breakpoint condition with a when.
#[derive(Clone, Debug, PartialEq)]
pub struct Term {
    pub seat: usize,
    pub key: String,
    test: Test,
    pub line: f64,
    pub when: When,
}

/// A Want or a Don't Want: a number about a seat, and how much it counts.
#[derive(Clone, Debug, PartialEq)]
pub struct Aim {
    pub seat: usize,
    pub key: String,
    pub weight: f64,
}

/// The four lists.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Objective {
    pub need: Vec<Term>,
    pub avoid: Vec<Term>,
    pub want: Vec<Aim>,
    pub dont_want: Vec<Aim>,
}

/// What one turn of a line read for each term, list by list.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Reading {
    pub turn: u32,
    #[serde(default)]
    pub need: Vec<bool>,
    #[serde(default)]
    pub avoid: Vec<bool>,
    #[serde(default)]
    pub want: Vec<Option<f64>>,
    #[serde(default)]
    pub dont_want: Vec<Option<f64>>,
}

/// What a line came to, term by term.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct Verdict {
    /// Whether each Need was met.
    pub need: Vec<bool>,
    /// Whether each Must Avoid was kept clear of.
    pub avoid: Vec<bool>,
    /// How many of the two lists were failed.
    pub broken: usize,
    /// The turn a Must Avoid ended the line on, if one did.
    pub killed: Option<u32>,
    /// What each Want and Don't Want read on the line's last turn.
    pub want: Vec<Option<f64>>,
    pub dont_want: Vec<Option<f64>>,
}

impl Verdict {
    pub fn feasible(&self) -> bool {
        self.broken == 0
    }
}

impl Objective {
    /// What a search is asked for when nothing is named: win, and do not lose
    /// (the review: "Win and loss become a default Need and Must Avoid").
    pub fn win_and_do_not_lose() -> Objective {
        Objective {
            need: vec![Term {
                seat: 0,
                key: "won".into(),
                test: Test::Equals,
                line: 1.0,
                when: When::End,
            }],
            avoid: vec![Term {
                seat: 0,
                key: "lost".into(),
                test: Test::Equals,
                line: 1.0,
                when: When::Always,
            }],
            want: Vec::new(),
            dont_want: Vec::new(),
        }
    }

    /// The four lists as a request names them:
    /// `{"need": [...], "avoid": [...], "want": [...], "dont_want": [...]}`.
    /// A term is `{"seat", "key", "test", "value", "when", "turn"}` and an
    /// aim `{"seat", "key", "weight"}`; the seat is 0, the played one, unless
    /// named. A request that names none of the four is
    /// [`Objective::win_and_do_not_lose`].
    pub fn parse(asked: &Value) -> Result<Objective, String> {
        let lists = ["need", "avoid", "want", "dont_want"];
        if !lists.iter().any(|list| asked.get(list).is_some()) {
            return Ok(Objective::win_and_do_not_lose());
        }
        let items = |list: &str| -> Result<Vec<&Value>, String> {
            let items: Vec<&Value> = match asked.get(list) {
                None | Some(Value::Null) => Vec::new(),
                Some(Value::Array(items)) => items.iter().collect(),
                Some(_) => return Err(format!("{list} is a list")),
            };
            if items.len() > MOST_TERMS {
                return Err(format!("at most {MOST_TERMS} terms in {list}"));
            }
            Ok(items)
        };
        let objective = Objective {
            need: items("need")?
                .into_iter()
                .map(Term::parse)
                .collect::<Result<_, _>>()?,
            avoid: items("avoid")?
                .into_iter()
                .map(Term::parse)
                .collect::<Result<_, _>>()?,
            want: items("want")?
                .into_iter()
                .map(Aim::parse)
                .collect::<Result<_, _>>()?,
            dont_want: items("dont_want")?
                .into_iter()
                .map(Aim::parse)
                .collect::<Result<_, _>>()?,
        };
        if objective.need.is_empty()
            && objective.avoid.is_empty()
            && objective.want.is_empty()
            && objective.dont_want.is_empty()
        {
            return Err("a search needs something in one of its four lists".to_string());
        }
        Ok(objective)
    }

    pub fn json(&self) -> Value {
        json!({
            "need": self.need.iter().map(Term::json).collect::<Vec<_>>(),
            "avoid": self.avoid.iter().map(Term::json).collect::<Vec<_>>(),
            "want": self.want.iter().map(Aim::json).collect::<Vec<_>>(),
            "dont_want": self.dont_want.iter().map(Aim::json).collect::<Vec<_>>(),
        })
    }

    /// Every term read back from where it lands before a line is played: a
    /// seat or a key this world has no number for would read nothing on
    /// every turn, and say nothing about why.
    pub fn check(&self, game: &Game) -> Result<(), String> {
        let mut reader = Reader::new(game);
        let keys = self
            .need
            .iter()
            .chain(&self.avoid)
            .map(|term| (term.seat, &term.key))
            .chain(
                self.want
                    .iter()
                    .chain(&self.dont_want)
                    .map(|aim| (aim.seat, &aim.key)),
            );
        for (seat, key) in keys {
            if seat >= game.players.len() {
                return Err(format!("there is no seat {seat}"));
            }
            if reader.number(seat, key).is_none() {
                return Err(format!("seat {seat} has no number called {key:?}"));
            }
        }
        Ok(())
    }

    /// What this world reads for every term.
    pub fn read(&self, game: &Game) -> Reading {
        let mut reader = Reader::new(game);
        let mut held = |term: &Term| {
            reader
                .number(term.seat, &term.key)
                .is_some_and(|now| term.test.holds(now, term.line))
        };
        let need = self.need.iter().map(&mut held).collect();
        let avoid = self.avoid.iter().map(&mut held).collect();
        Reading {
            turn: game.turn,
            need,
            avoid,
            want: self
                .want
                .iter()
                .map(|aim| reader.number(aim.seat, &aim.key))
                .collect(),
            dont_want: self
                .dont_want
                .iter()
                .map(|aim| reader.number(aim.seat, &aim.key))
                .collect(),
        }
    }

    /// The Must Avoid a turn's reading breaks for good, if any: one held
    /// always or ever, or by a turn this one has not passed. A line is ended
    /// there, as a breakpoint ends a run. A Must Avoid held only at the end,
    /// and every Need, cut nothing: a line can still come right.
    pub fn kills(&self, reading: &Reading) -> Option<usize> {
        self.avoid.iter().enumerate().find_map(|(at, term)| {
            let broken = reading.avoid.get(at).copied().unwrap_or(false);
            let binding = match term.when {
                When::End => false,
                When::Always | When::Ever => true,
                When::By(turn) => reading.turn <= turn,
            };
            (broken && binding).then_some(at)
        })
    }

    /// What a line came to, from what each turn it played read, in order
    /// (the turn it began on is not one of them: every line shares it).
    pub fn judge(&self, series: &[Reading]) -> Verdict {
        let last = series.last();
        let any = |at: usize, list: fn(&Reading) -> &Vec<bool>, by: Option<u32>| {
            series.iter().any(|reading| {
                by.is_none_or(|turn| reading.turn <= turn)
                    && list(reading).get(at).copied().unwrap_or(false)
            })
        };
        let need: Vec<bool> = self
            .need
            .iter()
            .enumerate()
            .map(|(at, term)| match term.when {
                When::End => last.is_some_and(|r| r.need.get(at).copied().unwrap_or(false)),
                When::Always => {
                    !series.is_empty()
                        && series
                            .iter()
                            .all(|r| r.need.get(at).copied().unwrap_or(false))
                }
                When::Ever => any(at, |r| &r.need, None),
                When::By(turn) => any(at, |r| &r.need, Some(turn)),
            })
            .collect();
        let avoid: Vec<bool> = self
            .avoid
            .iter()
            .enumerate()
            .map(|(at, term)| match term.when {
                When::End => !last.is_some_and(|r| r.avoid.get(at).copied().unwrap_or(false)),
                When::Always | When::Ever => !any(at, |r| &r.avoid, None),
                When::By(turn) => !any(at, |r| &r.avoid, Some(turn)),
            })
            .collect();
        let broken = need.iter().chain(&avoid).filter(|kept| !**kept).count();
        Verdict {
            need,
            avoid,
            broken,
            killed: series
                .iter()
                .find(|reading| self.kills(reading).is_some())
                .map(|reading| reading.turn),
            want: last.map_or_else(|| vec![None; self.want.len()], |r| r.want.clone()),
            dont_want: last
                .map_or_else(|| vec![None; self.dont_want.len()], |r| r.dont_want.clone()),
        }
    }

    /// How a line's Wants and Don't Wants stand against the baseline's, each
    /// as percent over it and signed so that more is better: a Don't Want
    /// that came out lower counts for the line. A number that is not there
    /// counts nothing.
    pub fn over(&self, line: &Verdict, base: &Verdict) -> Vec<f64> {
        let over = |now: Option<&Option<f64>>, then: Option<&Option<f64>>| match (now, then) {
            (Some(Some(now)), Some(Some(then))) => 100.0 * (now - then) / then.abs().max(1.0),
            _ => 0.0,
        };
        let wants = (0..self.want.len()).map(|at| over(line.want.get(at), base.want.get(at)));
        let unwanted = (0..self.dont_want.len())
            .map(|at| -over(line.dont_want.get(at), base.dont_want.get(at)));
        wants.chain(unwanted).collect()
    }

    /// The weighted sum of [`Objective::over`]: what the ranking reads after
    /// the Needs and Must Avoids.
    pub fn gain(&self, line: &Verdict, base: &Verdict) -> f64 {
        let weights = self
            .want
            .iter()
            .chain(&self.dont_want)
            .map(|aim| aim.weight);
        self.over(line, base)
            .into_iter()
            .zip(weights)
            .map(|(over, weight)| over * weight)
            .sum()
    }
}

impl Term {
    fn parse(asked: &Value) -> Result<Term, String> {
        let seat = seat_of(asked)?;
        let key = key_of(asked)?;
        let sign = asked["test"].as_str().unwrap_or(">=");
        let test = Test::parse(sign)
            .filter(|test| *test != Test::Changes)
            .ok_or_else(|| format!("a term's test is one of < <= > >= == !=, not {sign:?}"))?;
        let line = asked["value"]
            .as_f64()
            .filter(|line| line.is_finite())
            .ok_or_else(|| format!("{key} {sign} needs a number to hold it to"))?;
        Ok(Term {
            seat,
            key,
            test,
            line,
            when: When::parse(asked)?,
        })
    }

    fn json(&self) -> Value {
        let mut out = json!({
            "seat": self.seat,
            "key": self.key,
            "test": self.test.sign(),
            "value": self.line,
            "when": self.when.word(),
        });
        if let When::By(turn) = self.when {
            out["turn"] = json!(turn);
        }
        out
    }
}

impl Aim {
    fn parse(asked: &Value) -> Result<Aim, String> {
        let seat = seat_of(asked)?;
        let key = key_of(asked)?;
        let weight = match asked.get("weight") {
            None | Some(Value::Null) => 1.0,
            Some(weight) => weight
                .as_f64()
                .filter(|weight| weight.is_finite() && *weight != 0.0)
                .ok_or_else(|| format!("{key}'s weight is a number other than 0"))?,
        };
        Ok(Aim { seat, key, weight })
    }

    fn json(&self) -> Value {
        json!({"seat": self.seat, "key": self.key, "weight": self.weight})
    }
}

fn seat_of(asked: &Value) -> Result<usize, String> {
    match asked.get("seat") {
        None | Some(Value::Null) => Ok(0),
        Some(seat) => seat
            .as_u64()
            .and_then(|seat| usize::try_from(seat).ok())
            .ok_or_else(|| "a term names a civilization by its seat".to_string()),
    }
}

fn key_of(asked: &Value) -> Result<String, String> {
    asked["key"]
        .as_str()
        .map(str::trim)
        .filter(|key| !key.is_empty())
        .map(str::to_string)
        .ok_or_else(|| "a term names what it reads (its key)".to_string())
}

/// Numbers about seats, read off one world.
///
/// A key is looked up in the seat's row of the standings first
/// (`obs::standing`: score, cities, population, gold_per_turn and yields,
/// reached with a dot as in `yields.science`), which costs little. A key it
/// does not have is looked up in the seat's entry of the omniscient
/// observation, which is dear (60 to 80 ms of a 150 ms step at turn 80 of a
/// six-seat game, `debugger::Read`) and built at most once a turn. `won` and
/// `lost` are the game's own verdict: lost is the seat fallen, or the game
/// won by somebody else.
struct Reader<'a> {
    game: &'a Game,
    standings: BTreeMap<usize, Value>,
    observed: Option<Value>,
}

impl<'a> Reader<'a> {
    fn new(game: &'a Game) -> Reader<'a> {
        Reader {
            game,
            standings: BTreeMap::new(),
            observed: None,
        }
    }

    fn number(&mut self, seat: usize, key: &str) -> Option<f64> {
        let game = self.game;
        let player = game.players.get(seat)?;
        match key {
            "won" => return Some(f64::from(u8::from(game.winner == Some(seat)))),
            "lost" => {
                let lost = !player.alive || game.winner.is_some_and(|winner| winner != seat);
                return Some(f64::from(u8::from(lost)));
            }
            _ => {}
        }
        let dig = |from: &Value| -> Option<f64> {
            key.split('.')
                .try_fold(from, |held, part| held.get(part))
                .and_then(as_number)
        };
        let standing = self
            .standings
            .entry(seat)
            .or_insert_with(|| crate::obs::standing(game, seat));
        if let Some(number) = dig(standing) {
            return Some(number);
        }
        let observed = self
            .observed
            .get_or_insert_with(|| crate::obs::observation_spectator(game, 0));
        let row = observed["players"]
            .as_array()?
            .iter()
            .find(|row| row["id"] == json!(seat))?;
        dig(row)
    }
}

/// Which Civ AI holds the seat from which turn: `[[turn, name], ...]`, each
/// until the next one's turn, the first from the turn the search began on.
/// Two neighbours are never the same Civ AI.
pub type Plan = Vec<(u32, String)>;

fn tidy(plan: Plan) -> Plan {
    let mut tidy: Plan = Vec::with_capacity(plan.len());
    for (turn, name) in plan {
        if tidy.last().is_none_or(|(_, held)| *held != name) {
            tidy.push((turn, name));
        }
    }
    tidy
}

/// The Civ AI a plan has in the seat on a turn.
pub fn who(plan: &Plan, turn: u32) -> &str {
    let mut held = plan.first().map_or("", |(_, name)| name.as_str());
    for (from, name) in plan {
        if *from > turn {
            break;
        }
        held = name;
    }
    held
}

/// The plan with the turns from `from` up to `to` handed to `name`; `end` is
/// the turn the search plays to.
fn splice(plan: &Plan, from: u32, to: u32, end: u32, name: &str) -> Plan {
    let mut next: Plan = plan
        .iter()
        .filter(|(turn, _)| *turn < from)
        .cloned()
        .collect();
    next.push((from, name.to_string()));
    if to < end {
        next.push((to, who(plan, to).to_string()));
        next.extend(plan.iter().filter(|(turn, _)| *turn > to).cloned());
    }
    tidy(next)
}

/// A plan written as one string, the same for the same plan.
pub fn key(plan: &Plan) -> String {
    plan.iter()
        .map(|(turn, name)| format!("{turn}:{name}"))
        .collect::<Vec<_>>()
        .join(" ")
}

/// The strides the climb goes at, widest first: a quarter of the turns
/// played, halved down to a turn, and to no finer than a thirtieth of a long
/// search, whose lines are dear.
fn strides(turns: u32) -> Vec<u32> {
    let finest = ((f64::from(turns) / 30.0).round() as u32).max(1);
    let mut strides = Vec::new();
    let mut stride = ((f64::from(turns) / 4.0).round() as u32).max(finest);
    loop {
        if !strides.contains(&stride) {
            strides.push(stride);
        }
        if stride <= finest {
            break;
        }
        stride = ((f64::from(stride) / 2.0).round() as u32).max(finest);
    }
    strides
}

/// What a search is asked: the four lists, from which turn to which, the
/// Civ AIs it may hand the seat to (the alphabet) and the one seated, whose
/// line alone is what Wants are counted against.
#[derive(Clone, Debug, PartialEq)]
pub struct Spec {
    pub objective: Objective,
    pub from: u32,
    pub end: u32,
    pub letters: Vec<String>,
    pub seated: String,
    /// How many copies of the engine play lines at once: how far down its
    /// queue a free one looks for a line it already holds the start of.
    pub forks: usize,
}

impl Spec {
    /// `{"objective", "from", "turns" | "end", "letters", "seated", "forks"}`.
    pub fn parse(asked: &Value) -> Result<Spec, String> {
        let objective = Objective::parse(&asked["objective"])?;
        let from = asked["from"]
            .as_u64()
            .and_then(|turn| u32::try_from(turn).ok())
            .filter(|turn| *turn > 0)
            .ok_or_else(|| "a search begins from a turn (from)".to_string())?;
        let end = match (asked["end"].as_u64(), asked["turns"].as_u64()) {
            (Some(end), _) => u32::try_from(end).unwrap_or(u32::MAX),
            (None, Some(turns)) => from.saturating_add(u32::try_from(turns).unwrap_or(u32::MAX)),
            (None, None) => return Err("a search plays to a turn (end, or turns)".to_string()),
        };
        if end <= from || end - from > MOST_TURNS {
            return Err(format!(
                "a search plays 1 to {MOST_TURNS} turns ahead of the turn it begins on"
            ));
        }
        let mut letters: Vec<String> = Vec::new();
        for letter in asked["letters"].as_array().into_iter().flatten() {
            let letter = letter.as_str().unwrap_or("").trim();
            if !letter.is_empty() && !letters.iter().any(|held| held == letter) {
                letters.push(letter.to_string());
            }
        }
        let seated = asked["seated"]
            .as_str()
            .map(str::trim)
            .filter(|seated| !seated.is_empty())
            .map(str::to_string)
            .or_else(|| letters.first().cloned())
            .ok_or_else(|| "a search names the Civ AIs it may seat".to_string())?;
        if !letters.contains(&seated) {
            letters.insert(0, seated.clone());
        }
        if letters.len() > MOST_LETTERS {
            return Err(format!("a search seats at most {MOST_LETTERS} Civ AIs"));
        }
        let forks = asked["forks"].as_u64().unwrap_or(1).clamp(1, 256) as usize;
        Ok(Spec {
            objective,
            from,
            end,
            letters,
            seated,
            forks,
        })
    }
}

impl Spec {
    /// Every Civ AI the search may seat, built once before any line is
    /// played: a name the engine cannot build (no such roster row, a gene
    /// it does not have, a target on a scripted Civ AI) is refused now
    /// rather than on every line that names it.
    pub fn check_letters(&self) -> Result<(), String> {
        for letter in &self.letters {
            crate::strategies::build_named(letter).map_err(|error| format!("{letter}: {error}"))?;
        }
        Ok(())
    }
}

/// One line played, and what it came to.
#[derive(Clone, Debug, PartialEq)]
pub struct Line {
    pub plan: Plan,
    pub key: String,
    /// The turn it was played to: the search's last, or where the game
    /// ended or a Must Avoid ended the line.
    pub last: u32,
    pub series: Vec<Reading>,
    pub verdict: Verdict,
    /// Against the seated Civ AI's line alone ([`Objective::gain`]); 0 until
    /// that line is in.
    pub gain: f64,
    /// Term by term against the same ([`Objective::over`]).
    pub over: Vec<f64>,
}

impl Line {
    fn json(&self, objective: &Objective, series: bool) -> Value {
        let mut out = json!({
            "plan": self.plan,
            "key": self.key,
            "last": self.last,
            "verdict": self.verdict,
            "feasible": self.verdict.feasible(),
            "gain": self.gain,
            "over": self.over,
        });
        // The first Want, turn by turn, for the page's chart.
        if series && !objective.want.is_empty() {
            out["chart"] = json!(self
                .series
                .iter()
                .map(|reading| json!([reading.turn, reading.want.first().copied().flatten()]))
                .collect::<Vec<_>>());
        }
        out
    }
}

/// The climb. See the module's account.
pub struct Search {
    spec: Spec,
    strides: Vec<u32>,
    /// Which stride the climb is at.
    level: usize,
    queue: VecDeque<Plan>,
    /// Every plan played, being played or waiting, by its key.
    seen: HashSet<String>,
    /// The plans of each Civ AI alone not yet in: the climb waits for them.
    sweep: HashSet<String>,
    /// How many copies a plan has failed on.
    tries: HashMap<String, u32>,
    lines: Vec<Line>,
    by_key: HashMap<String, usize>,
    baseline: Option<usize>,
    best: Option<usize>,
    front: Vec<usize>,
    /// Lines put in from outside the climb (`offer`).
    offered: usize,
    played: u64,
}

impl Search {
    pub fn new(spec: Spec) -> Search {
        let mut search = Search {
            strides: strides(spec.end - spec.from),
            spec,
            level: 0,
            queue: VecDeque::new(),
            seen: HashSet::new(),
            sweep: HashSet::new(),
            tries: HashMap::new(),
            lines: Vec::new(),
            by_key: HashMap::new(),
            baseline: None,
            best: None,
            front: Vec::new(),
            offered: 0,
            played: 0,
        };
        // First every Civ AI alone, the seated one among them: the climb
        // begins once those are in.
        for letter in search.spec.letters.clone() {
            let alone = vec![(search.spec.from, letter)];
            search.sweep.insert(key(&alone));
            search.queue_plan(alone);
        }
        search
    }

    pub fn spec(&self) -> &Spec {
        &self.spec
    }

    fn queue_plan(&mut self, plan: Plan) -> bool {
        let plan = tidy(plan);
        if !self.seen.insert(key(&plan)) {
            return false;
        }
        self.queue.push_back(plan);
        true
    }

    /// Put a line in from outside the climb: a person's, or a proposer's
    /// (the review: "The model reads the state and seeds 20 candidate lines
    /// into the queue, and the simulator verifies them exactly"). It is
    /// played and judged like any other, and the climb grows from it if it
    /// earns a place. A plan that names a Civ AI the search does not seat, or
    /// a turn outside it, is refused.
    pub fn offer(&mut self, plan: Plan) -> Result<bool, String> {
        let mut plan = plan;
        plan.sort_by_key(|(turn, _)| *turn);
        let Some(first) = plan.first_mut() else {
            return Err("a line names at least one Civ AI".to_string());
        };
        if first.0 > self.spec.from {
            return Err(format!("a line begins on turn {}", self.spec.from));
        }
        first.0 = self.spec.from;
        for (turn, name) in &plan {
            if *turn >= self.spec.end {
                return Err(format!(
                    "turn {turn} is past the search's last, {}",
                    self.spec.end
                ));
            }
            if !self.spec.letters.contains(name) {
                return Err(format!("this search does not seat {name:?}"));
            }
        }
        let queued = self.queue_plan(plan);
        if queued {
            self.offered += 1;
        }
        Ok(queued)
    }

    /// The turn a copy holding `holding`, played to turn `at`, and `plan`
    /// part on: up to there the copy holds the plan's turns already, and it
    /// walks back to there to play it.
    pub fn parts(&self, holding: Option<&Plan>, at: u32, plan: &Plan) -> u32 {
        let mut turn = self.spec.from;
        if let Some(holding) = holding {
            while turn < at && turn < self.spec.end && who(holding, turn) == who(plan, turn) {
                turn += 1;
            }
        }
        turn
    }

    /// The next line for a copy that is free: of the ones at the head of the
    /// queue, the one it holds the most turns of already. None when there is
    /// nothing to play for now; [`Search::done`] says whether there ever will
    /// be.
    pub fn next(&mut self, holding: Option<&Plan>, at: u32) -> Option<Plan> {
        if self.queue.is_empty() && self.sweep.is_empty() {
            self.grow();
        }
        let look = self.queue.len().min(4 * self.spec.forks);
        let mut pick = None;
        let mut most = 0;
        for (index, plan) in self.queue.iter().take(look).enumerate() {
            let shared = self.parts(holding, at, plan);
            if pick.is_none() || shared > most {
                most = shared;
                pick = Some(index);
            }
        }
        pick.and_then(|index| self.queue.remove(index))
    }

    /// Nothing waits to be played, and the climb has nothing finer to try.
    pub fn done(&mut self) -> bool {
        if self.queue.is_empty() && self.sweep.is_empty() {
            self.grow();
        }
        self.queue.is_empty() && self.sweep.is_empty()
    }

    /// A line played: what each turn of it read, from the turn after the
    /// search's first, and the turn it was played to.
    pub fn took(&mut self, plan: Plan, series: Vec<Reading>, last: u32) {
        let plan = tidy(plan);
        let key = key(&plan);
        self.played += u64::from(last.saturating_sub(self.spec.from));
        let verdict = self.spec.objective.judge(&series);
        let index = self.lines.len();
        self.lines.push(Line {
            plan,
            key: key.clone(),
            last,
            series,
            verdict,
            gain: 0.0,
            over: Vec::new(),
        });
        self.by_key.insert(key.clone(), index);
        if self.lines[index].plan.len() == 1 && self.lines[index].plan[0].1 == self.spec.seated {
            self.baseline = Some(index);
        }
        self.sweep.remove(&key);
        self.rank();
    }

    /// A line a copy could not play: it goes back in the queue once, and a
    /// line that fails a second copy is let go.
    pub fn failed(&mut self, plan: Plan) {
        let plan = tidy(plan);
        let key = key(&plan);
        let tries = self.tries.entry(key.clone()).or_insert(0);
        *tries += 1;
        if *tries < 2 {
            self.queue.push_back(plan);
        } else {
            self.sweep.remove(&key);
        }
    }

    /// Which of two lines is the better: the one that meets every Need and
    /// breaks no Must Avoid; then the one that fails fewer of them; then the
    /// more of what is Wanted less what is not, against the seated Civ AI
    /// alone; then the one with fewer handovers.
    pub fn better(one: &Line, other: &Line) -> bool {
        if one.verdict.feasible() != other.verdict.feasible() {
            return one.verdict.feasible();
        }
        if one.verdict.broken != other.verdict.broken {
            return one.verdict.broken < other.verdict.broken;
        }
        if one.gain != other.gain {
            return one.gain > other.gain;
        }
        one.plan.len() < other.plan.len()
    }

    /// Whether `one` is at least as good as `other` on every term and better
    /// on one: fewer Needs and Must Avoids failed, and each Want and Don't
    /// Want against the baseline.
    fn dominates(one: &Line, other: &Line) -> bool {
        let mine = std::iter::once(-(one.verdict.broken as f64)).chain(one.over.iter().copied());
        let theirs =
            std::iter::once(-(other.verdict.broken as f64)).chain(other.over.iter().copied());
        let mut ahead = false;
        for (mine, theirs) in mine.zip(theirs) {
            if mine < theirs {
                return false;
            }
            ahead |= mine > theirs;
        }
        ahead
    }

    /// Every line's standing again, now that one more is in. Until the
    /// seated Civ AI's line alone is in, nothing is counted against it and
    /// no line is the best.
    fn rank(&mut self) {
        let Some(base) = self.baseline.map(|at| self.lines[at].verdict.clone()) else {
            return;
        };
        let objective = &self.spec.objective;
        for line in &mut self.lines {
            line.over = objective.over(&line.verdict, &base);
            line.gain = objective.gain(&line.verdict, &base);
        }
        let before = self.best.map(|at| self.lines[at].key.clone());
        let mut best = 0;
        for at in 1..self.lines.len() {
            if Search::better(&self.lines[at], &self.lines[best]) {
                best = at;
            }
        }
        self.best = Some(best);
        self.front = (0..self.lines.len())
            .filter(|&at| {
                !self
                    .lines
                    .iter()
                    .any(|other| Search::dominates(other, &self.lines[at]))
            })
            .collect();
        self.front.sort_by(|&one, &other| {
            if Search::better(&self.lines[one], &self.lines[other]) {
                std::cmp::Ordering::Less
            } else if Search::better(&self.lines[other], &self.lines[one]) {
                std::cmp::Ordering::Greater
            } else {
                std::cmp::Ordering::Equal
            }
        });
        // What is waiting was made around lines a new best has passed: it is
        // let go, and the climb begins again around the new one, at its
        // widest stride.
        let now = Some(self.lines[best].key.clone());
        if before.is_some() && now != before && self.sweep.is_empty() {
            for plan in std::mem::take(&mut self.queue) {
                self.seen.remove(&key(&plan));
            }
            self.level = 0;
        }
    }

    /// More plans to play, around the best line and the rest of the front:
    /// at each of a spread of turns, each other Civ AI is handed the rest of
    /// the line, and one stride of it. The latest turns come first, because
    /// a copy plays those soonest. When a stride has nothing new to offer the
    /// next finer one is tried.
    fn grow(&mut self) -> usize {
        let Some(best) = self.best else {
            return 0;
        };
        let mut seeds = vec![best];
        seeds.extend(
            self.front
                .iter()
                .copied()
                .filter(|&at| at != best)
                .take(FRONT_SEEDS),
        );
        let (from, end) = (self.spec.from, self.spec.end);
        while self.level < self.strides.len() {
            let stride = self.strides[self.level];
            let mut added = 0;
            for &seed in &seeds {
                let (plan, last) = (self.lines[seed].plan.clone(), self.lines[seed].last);
                if last <= from {
                    continue;
                }
                let mut turn = from + stride * ((last - 1 - from) / stride);
                loop {
                    let holds = who(&plan, turn).to_string();
                    for name in self.spec.letters.clone() {
                        if name == holds {
                            continue;
                        }
                        added += usize::from(self.queue_plan(splice(&plan, turn, end, end, &name)));
                        if turn + stride < last {
                            added += usize::from(self.queue_plan(splice(
                                &plan,
                                turn,
                                turn + stride,
                                end,
                                &name,
                            )));
                        }
                    }
                    if turn < from + stride {
                        break;
                    }
                    turn -= stride;
                }
            }
            if added > 0 {
                return added;
            }
            self.level += 1;
        }
        0
    }

    pub fn best(&self) -> Option<&Line> {
        self.best.map(|at| &self.lines[at])
    }

    pub fn baseline(&self) -> Option<&Line> {
        self.baseline.map(|at| &self.lines[at])
    }

    /// The lines nothing beats on every term at once, the best first.
    pub fn front(&self) -> impl Iterator<Item = &Line> {
        self.front.iter().map(|&at| &self.lines[at])
    }

    pub fn lines(&self) -> &[Line] {
        &self.lines
    }

    /// What the search has found, for a page or a log.
    pub fn report(&self) -> Value {
        let objective = &self.spec.objective;
        let feasible = self
            .lines
            .iter()
            .filter(|line| line.verdict.feasible())
            .count();
        let alone: Vec<Value> = self
            .lines
            .iter()
            .filter(|line| line.plan.len() == 1)
            .map(|line| line.json(objective, false))
            .collect();
        json!({
            "objective": objective.json(),
            "from": self.spec.from,
            "end": self.spec.end,
            "letters": self.spec.letters,
            "seated": self.spec.seated,
            "lines": self.lines.len(),
            "played": self.played,
            "queued": self.queue.len(),
            "offered": self.offered,
            "feasible": feasible,
            "stride": self.strides.get(self.level),
            "best": self.best().map(|line| line.json(objective, true)),
            "baseline": self.baseline().map(|line| line.json(objective, true)),
            "front": self.front().take(FRONT_SAID).map(|line| line.json(objective, false)).collect::<Vec<_>>(),
            "front_size": self.front.len(),
            "alone": alone,
        })
    }
}

/// `POST /autosearch`: the climb, for a page that plays its lines on copies
/// of the engine. `op` is one of:
///
/// - `begin` with a [`Spec`]: a new search, in place of any before it;
/// - `offer` with `plans`: lines put in from outside the climb;
/// - `next` with the plan a copy holds (`holding`) and the turn it is on
///   (`at`): the line for it to play, or none for now (`done` says whether
///   for good);
/// - `took` with `plan`, `series` and `last`: what a line came to;
/// - `failed` with `plan`: a line a copy could not play;
/// - `report`, and `end`, which lets the search go.
///
/// Every answer but `next`'s carries the report.
pub fn route(held: &mut Option<Search>, asked: &Value) -> Value {
    let plan_of = |value: &Value| -> Option<Plan> {
        serde_json::from_value::<Plan>(value.clone())
            .ok()
            .filter(|plan| !plan.is_empty())
    };
    let op = asked["op"].as_str().unwrap_or("report");
    if op == "begin" {
        return match Spec::parse(asked).and_then(|spec| spec.check_letters().map(|()| spec)) {
            Ok(spec) => {
                let search = Search::new(spec);
                let report = search.report();
                *held = Some(search);
                report
            }
            Err(error) => json!({"error": error}),
        };
    }
    if op == "end" {
        *held = None;
        return json!({"ended": true});
    }
    let Some(search) = held.as_mut() else {
        return json!({"error": "no search has begun"});
    };
    match op {
        "next" => {
            let holding = plan_of(&asked["holding"]);
            let at = asked["at"]
                .as_u64()
                .and_then(|turn| u32::try_from(turn).ok())
                .unwrap_or(search.spec.from);
            let plan = search.next(holding.as_ref(), at);
            let from = plan
                .as_ref()
                .map(|plan| search.parts(holding.as_ref(), at, plan));
            let done = plan.is_none() && search.done();
            json!({"plan": plan, "from": from, "done": done})
        }
        "took" => {
            let Some(plan) = plan_of(&asked["plan"]) else {
                return json!({"error": "took names the plan that was played"});
            };
            let series: Vec<Reading> =
                serde_json::from_value(asked["series"].clone()).unwrap_or_default();
            let last = asked["last"]
                .as_u64()
                .and_then(|turn| u32::try_from(turn).ok())
                .unwrap_or_else(|| series.last().map_or(search.spec.from, |r| r.turn));
            search.took(plan, series, last);
            search.report()
        }
        "failed" => {
            if let Some(plan) = plan_of(&asked["plan"]) {
                search.failed(plan);
            }
            search.report()
        }
        "offer" => {
            let mut out = Vec::new();
            for plan in asked["plans"].as_array().into_iter().flatten() {
                out.push(match plan_of(plan).map(|plan| search.offer(plan)) {
                    Some(Ok(queued)) => json!(queued),
                    Some(Err(error)) => json!(error),
                    None => json!("not a plan"),
                });
            }
            let mut report = search.report();
            report["offers"] = json!(out);
            report
        }
        "report" => search.report(),
        other => json!({"error": format!("no autosearch operation named {other:?}")}),
    }
}

/// What the native runner is asked (`civvis autosearch`).
#[derive(Clone, Debug)]
pub struct RunOptions {
    /// The world: the site's opening world on this seed.
    pub seed: u64,
    /// The search, as [`Spec::parse`] reads it; `from` and `end` are turns of
    /// that world, which the seated Civ AI plays up to `from` first.
    pub spec: Value,
    /// Threads, each with a copy of the engine.
    pub cores: usize,
    /// Lines to put in before the climb (`Search::offer`).
    pub offers: Vec<Plan>,
    /// Other worlds to play the best line on afterwards, against the seated
    /// Civ AI alone on each: the robust mode.
    pub robust: Vec<u64>,
    /// The table's Fog-honest AI setting, when the engine has one.
    pub fog_honest: Option<bool>,
}

fn ask(session: &mut Session, request: Value) -> Result<Value, String> {
    let answer = session.debug(&request);
    match answer["error"].as_str() {
        Some(error) => Err(error.to_string()),
        None => Ok(answer),
    }
}

/// A world of the site's on `seed`, with a seat of its own for the debugger,
/// played by `seated` up to turn `from`, with the objective set.
fn world(seed: u64, spec: &Spec, fog_honest: Option<bool>) -> Result<Session, String> {
    let mut params = super::stock_opening_params(seed);
    params.spectate = false;
    let mut session = Session::new(params);
    if let Some(on) = fog_honest {
        let _ = on; // TEMP-FOG session.set_fog_honest(on);
    }
    ask(
        &mut session,
        json!({"op": "status", "strategy": spec.seated, "lean": true}),
    )?;
    while session.game.turn < spec.from {
        let turns = (spec.from - session.game.turn).min(100);
        let answer = ask(
            &mut session,
            json!({"op": "play", "turns": turns, "strategy": spec.seated, "lean": true}),
        )?;
        if answer["finished"] == json!(true) {
            return Err(format!(
                "the game on seed {seed} ended before turn {}",
                spec.from
            ));
        }
    }
    ask(
        &mut session,
        json!({"op": "objective", "objective": spec.objective.json(), "lean": true}),
    )?;
    Ok(session)
}

/// A copy of the engine and the line it holds.
struct Copy {
    session: Session,
    plan: Option<Plan>,
    at: u32,
    series: Vec<Reading>,
}

impl Copy {
    /// Play `plan` from turn `from` (where it parts from the line this copy
    /// holds) to the search's end: walk back there with the plan's Civ AI
    /// named, so that the agents kept with the turn sit down again if they are
    /// the same, and play up a stretch at a time. Ends early where the game
    /// does or a Must Avoid ends the line.
    fn play(&mut self, spec: &Spec, plan: &Plan, from: u32) -> Result<u32, String> {
        let mut turn = from;
        let there = ask(
            &mut self.session,
            json!({"op": "jump", "turn": turn, "strategy": who(plan, turn), "lean": true}),
        )?;
        let landed = there["turn"].as_u64().unwrap_or(0) as u32;
        if landed != turn {
            // A turn that is no longer kept answers with the nearest before it.
            if landed < spec.from {
                return Err(format!("turn {} is no longer kept", spec.from));
            }
            turn = landed;
            ask(
                &mut self.session,
                json!({"op": "jump", "turn": turn, "strategy": who(plan, turn), "lean": true}),
            )?;
        }
        self.plan = Some(plan.clone());
        self.at = turn;
        self.series.truncate((turn - spec.from) as usize);
        while self.at < spec.end {
            let stop = plan
                .iter()
                .map(|(start, _)| *start)
                .find(|start| *start > self.at)
                .unwrap_or(spec.end)
                .min(spec.end);
            let asked = CHUNK.min(stop - self.at);
            let answer = ask(
                &mut self.session,
                json!({"op": "play", "turns": asked, "strategy": who(plan, self.at), "lean": true}),
            )?;
            let terms: Vec<Reading> =
                serde_json::from_value(answer["terms"].clone()).unwrap_or_default();
            let played = terms.len() as u32;
            self.series.extend(terms);
            self.at += played;
            if answer["finished"] == json!(true) || !answer["killed"].is_null() || played < asked {
                break;
            }
        }
        Ok(self.at)
    }
}

/// The native headless runner: a search on this process's own copies of the
/// engine, one a thread, and then, in robust mode, the best line played on
/// other worlds. The answer is the search's report, with `robust` beside it.
pub fn run(options: &RunOptions) -> Result<Value, String> {
    let spec = Spec::parse(&options.spec)?;
    spec.check_letters()?;
    let root = world(options.seed, &spec, options.fog_honest)?;
    let cores = options.cores.clamp(1, 256);
    let mut search = Search::new(Spec {
        forks: cores,
        ..spec.clone()
    });
    for plan in &options.offers {
        search.offer(plan.clone())?;
    }
    let copies: Vec<Copy> = (0..cores)
        .map(|_| {
            let mut session = root.fork();
            // A copy keeps its agents with every turn it marks, so a line
            // played from the middle of another is the line played straight
            // through (`debug_remember`).
            session.debug(&json!({"op": "remember", "lean": true}));
            Copy {
                session,
                plan: None,
                at: spec.from,
                series: Vec::new(),
            }
        })
        .collect();
    // The search and how many copies are playing, under one lock: a copy
    // that finds nothing to play waits while another is playing, because
    // what that one brings back may give the climb more to try.
    let shared = Mutex::new((search, 0usize));
    let woken = Condvar::new();
    std::thread::scope(|scope| {
        for mut copy in copies {
            let (shared, woken, spec) = (&shared, &woken, &spec);
            scope.spawn(move || loop {
                let next = {
                    let mut held = shared.lock().unwrap_or_else(|poison| poison.into_inner());
                    loop {
                        let (search, busy) = &mut *held;
                        if let Some(plan) = search.next(copy.plan.as_ref(), copy.at) {
                            let from = search.parts(copy.plan.as_ref(), copy.at, &plan);
                            *busy += 1;
                            break Some((plan, from));
                        }
                        if *busy == 0 && search.done() {
                            woken.notify_all();
                            break None;
                        }
                        held = woken
                            .wait(held)
                            .unwrap_or_else(|poison| poison.into_inner());
                    }
                };
                let Some((plan, from)) = next else {
                    return;
                };
                let played = copy.play(spec, &plan, from);
                let mut held = shared.lock().unwrap_or_else(|poison| poison.into_inner());
                let (search, busy) = &mut *held;
                match played {
                    Ok(last) => search.took(plan, copy.series.clone(), last),
                    Err(error) => {
                        eprintln!("[autosearch] a copy failed: {error}");
                        copy.plan = None;
                        search.failed(plan);
                    }
                }
                *busy -= 1;
                woken.notify_all();
            });
        }
    });
    let (search, _) = shared
        .into_inner()
        .unwrap_or_else(|poison| poison.into_inner());
    let mut report = search.report();
    if !options.robust.is_empty() {
        report["robust"] = robust(&search, options)?;
    }
    Ok(report)
}

/// The robust mode: the best line, and the seated Civ AI alone, played on
/// each other world from the same turn, judged by the same lists. How often
/// the best line meets every Need there is the answer to whether it was a
/// fact about one world.
fn robust(search: &Search, options: &RunOptions) -> Result<Value, String> {
    let spec = search.spec();
    let Some(best) = search.best() else {
        return Ok(json!({"error": "the search found no line to play again"}));
    };
    let alone: Plan = vec![(spec.from, spec.seated.clone())];
    let mut worlds = Vec::new();
    let (mut met, mut met_alone, mut gains) = (0usize, 0usize, Vec::new());
    for &seed in &options.robust {
        let root = match world(seed, spec, options.fog_honest) {
            Ok(root) => root,
            Err(error) => {
                worlds.push(json!({"seed": seed, "error": error}));
                continue;
            }
        };
        let mut lines = Vec::new();
        for plan in [&alone, &best.plan] {
            let mut copy = Copy {
                session: root.fork(),
                plan: None,
                at: spec.from,
                series: Vec::new(),
            };
            copy.session.debug(&json!({"op": "remember", "lean": true}));
            copy.play(spec, plan, spec.from)?;
            lines.push(spec.objective.judge(&copy.series));
        }
        let (base, line) = (&lines[0], &lines[1]);
        let gain = spec.objective.gain(line, base);
        met += usize::from(line.feasible());
        met_alone += usize::from(base.feasible());
        gains.push(gain);
        worlds.push(json!({"seed": seed, "best": line, "alone": base, "gain": gain}));
    }
    let played = gains.len();
    let mean = if played > 0 {
        gains.iter().sum::<f64>() / played as f64
    } else {
        0.0
    };
    Ok(json!({
        "plan": best.plan,
        "worlds": worlds,
        "played": played,
        "needs_met": met,
        "needs_met_alone": met_alone,
        "mean_gain": mean,
    }))
}

#[cfg(test)]
#[path = "autosearch_tests.rs"]
mod tests;
