//! The simulator debugger: a played game as a timeline that can be walked.
//!
//! A game somebody is playing used to run one way. This module keeps a mark of
//! the world at the top of every turn the played seat has opened, and that one
//! record is what the debugger's controls are made of:
//!
//! * **back / forward / jump** put a marked turn on the table again. Going
//!   back does not forget the turns ahead: they stay marked until the world
//!   is played forward differently, and then they are replaced. A turn ahead
//!   is walked to only under the Civ AI that played it (`Timeline::line`):
//!   with another one seated it is that other agent's game, and going on
//!   from here plays, with the one that is seated.
//! * **undo** takes back the played seat's last action inside the open turn,
//!   by replaying the others on the turn's mark. The engine is deterministic
//!   and the random stream is part of the game, so the replay is exact.
//! * **step** plays one whole turn with the seat's agent and then asks the
//!   breakpoints whether to stop: a turn number, a kind of event, a phrase in
//!   the event log.
//! * **step in** takes the agent's turn one action at a time: the agent plans
//!   the turn on a copy of the world, and each step applies the next action
//!   of that plan to the real one. When the plan is spent, the step ends the
//!   turn.
//! * **reveal** lifts the fog from what the page is shown. The seat still
//!   plays by its own rules; only the observation changes.
//! * **step seat** takes the round apart instead: the seat's turn ends, and
//!   then each civilization, city-state and the barbarians play one at a
//!   time, with the world shown between them.
//! * **layers** are the rungs between playing every decision by hand and
//!   handing the seat over: research, cities, the empire's standing choices,
//!   diplomacy, units. The agent plans the turn on a copy of the world, and
//!   only the actions in a layer that is switched on are applied to the real
//!   one. The rest of the turn is left to the person.
//!
//! * **save points** keep a world under a name, to come back to whatever is
//!   played from here on. Going to one puts its world on the table with the
//!   turns of its own history, and keeps the world that was left.
//!
//! A mark is a clone of the game. The game shares what a turn did not change
//! with its clones, so a mark costs what its turn changed rather than the
//! world. That is still megabytes (`ROOMY_MARKS` has the measurement), so
//! every turn keeps its mark only for as long as there is room
//! (`ROOMY_MARKS`, and in a browser tab the memory line, because a tab's
//! memory only grows). When room runs out the oldest turns are thinned first
//! (`Timeline::thin`). Playing a thinned turn again would not bring it back:
//! a turn played by hand cannot be played by an agent, and an agent decides
//! things its logged actions do not carry.
//!
//! A save carries the world and not this record: a loaded game starts a
//! timeline of its own from the turn it resumes.
//!
//! Every mark also carries a print of its world (`world_print`), a number
//! two marks share only when they hold the same world. The two halves of a
//! split are played apart, and their prints are how the page knows up to
//! which turn they are still one game (`debug.worlds`).

use std::collections::{BTreeSet, VecDeque};
use std::sync::{Arc, Mutex};

use serde_json::{json, Value};

use super::Session;
use crate::ai::Ai;
use crate::game::{Action, Event, Game};

/// The newest marks are kept one a turn when older ones have to go, for as
/// long as there is room for twice as many.
const DENSE_MARKS: usize = 60;
/// Breakpoint phrases a page may hold at once.
const MAX_PHRASES: usize = 16;
/// Breakpoint turns a page may hold at once.
const MAX_BREAK_TURNS: usize = 64;
/// Breakpoint conditions a page may hold at once.
const MAX_CONDITIONS: usize = 16;
/// The most turns one `play` plays. The engine answers nothing else until it
/// is done, so a longer run is asked for in pieces.
const PLAY_MOST: u32 = 100;

/// One rung of handing the seat to its agent.
///
/// The order is the order the page lists them in, lightest first: a person
/// gives up research long before they give up their units.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Layer {
    Research,
    Cities,
    Empire,
    Diplomacy,
    Units,
}

impl Layer {
    pub const ALL: [Layer; 5] = [
        Layer::Research,
        Layer::Cities,
        Layer::Empire,
        Layer::Diplomacy,
        Layer::Units,
    ];

    pub fn id(self) -> &'static str {
        match self {
            Layer::Research => "research",
            Layer::Cities => "cities",
            Layer::Empire => "empire",
            Layer::Diplomacy => "diplomacy",
            Layer::Units => "units",
        }
    }

    fn label(self) -> &'static str {
        match self {
            Layer::Research => "Research and civics",
            Layer::Cities => "City production and purchases",
            Layer::Empire => "Government, policies, religion, governors, Great People",
            Layer::Diplomacy => "Diplomacy, envoys, spies",
            Layer::Units => "Units",
        }
    }

    fn parse(id: &str) -> Option<Layer> {
        Layer::ALL.into_iter().find(|layer| layer.id() == id)
    }

    /// The layer an action belongs to. `None` is the end of the turn, which
    /// no layer owns: a person who keeps any decision keeps that one.
    pub fn of(action: &Action) -> Option<Layer> {
        Some(match action {
            Action::EndTurn => return None,
            Action::Research { .. } | Action::Civic { .. } => Layer::Research,
            Action::Produce { .. }
            | Action::Buy { .. }
            | Action::BuyBuilding { .. }
            | Action::BuyDistrict { .. }
            | Action::BuyPlot { .. }
            | Action::KeepCity { .. }
            | Action::RazeCity { .. }
            | Action::LiberateCity { .. } => Layer::Cities,
            Action::Government { .. }
            | Action::SlotPolicy { .. }
            | Action::UnslotPolicy { .. }
            | Action::ChooseDedication { .. }
            | Action::ChoosePantheon { .. }
            | Action::ChooseSecretSociety { .. }
            | Action::EvangelizeBelief { .. }
            | Action::AppointGovernor { .. }
            | Action::AssignGovernor { .. }
            | Action::ReassignGovernor { .. }
            | Action::PromoteGovernor { .. }
            | Action::RecruitGreatPerson { .. }
            | Action::PatronizeGreatPerson { .. } => Layer::Empire,
            Action::DeclareWar { .. }
            | Action::DeclareWarWithCasusBelli { .. }
            | Action::MakePeace { .. }
            | Action::Denounce { .. }
            | Action::SendDelegation { .. }
            | Action::SendEmbassy { .. }
            | Action::ProposeDefensivePact { .. }
            | Action::ProposeJointWar { .. }
            | Action::RequestPromise { .. }
            | Action::DemandGold { .. }
            | Action::ProposeDeal { .. }
            | Action::AcceptDeal { .. }
            | Action::RejectDeal { .. }
            | Action::Trade { .. }
            | Action::CongressVote { .. }
            | Action::SendEnvoy { .. }
            | Action::LevyMilitary { .. }
            | Action::AssignSpy { .. }
            | Action::SpyMission { .. }
            | Action::PromoteSpy { .. } => Layer::Diplomacy,
            // Everything else is done by something standing on the map.
            _ => Layer::Units,
        })
    }
}

/// How a condition's number is held against its line.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum Test {
    Below,
    AtMost,
    Above,
    AtLeast,
    Equals,
    Differs,
    /// No line at all: the value is not what it was a turn ago.
    Changes,
}

impl Test {
    const ALL: [Test; 7] = [
        Test::Below,
        Test::AtMost,
        Test::Above,
        Test::AtLeast,
        Test::Equals,
        Test::Differs,
        Test::Changes,
    ];

    pub(super) fn sign(self) -> &'static str {
        match self {
            Test::Below => "<",
            Test::AtMost => "<=",
            Test::Above => ">",
            Test::AtLeast => ">=",
            Test::Equals => "==",
            Test::Differs => "!=",
            Test::Changes => "changes",
        }
    }

    pub(super) fn parse(sign: &str) -> Option<Test> {
        Test::ALL.into_iter().find(|test| test.sign() == sign)
    }

    pub(super) fn holds(self, now: f64, line: f64) -> bool {
        match self {
            Test::Below => now < line,
            Test::AtMost => now <= line,
            Test::Above => now > line,
            Test::AtLeast => now >= line,
            Test::Equals => now == line,
            Test::Differs => now != line,
            Test::Changes => false,
        }
    }
}

/// A breakpoint on the state of the game rather than on its log: one thing
/// the observation says about one civilization, held against a line.
///
/// What it reads is that civilization's entry in the omniscient observation
/// (`players[seat][key]`), so anything a spectator of the whole table is
/// told can be a condition, for any seat, without a list kept here. A number
/// is read as itself, a yes or no as 1 or 0, and a list as its length.
#[derive(Clone, Debug, PartialEq)]
struct Condition {
    seat: usize,
    key: String,
    test: Test,
    line: f64,
}

impl Condition {
    fn json(&self) -> Value {
        json!({
            "seat": self.seat,
            "key": self.key,
            "test": self.test.sign(),
            "value": self.line,
        })
    }
}

/// A reading as a number, when it is something that can be held to a line.
pub(super) fn as_number(reading: &Value) -> Option<f64> {
    match reading {
        Value::Number(number) => number.as_f64(),
        Value::Bool(yes) => Some(f64::from(u8::from(*yes))),
        Value::Array(list) => Some(list.len() as f64),
        Value::Object(map) => Some(map.len() as f64),
        _ => None,
    }
}

/// What each condition reads right now. `None` is a seat or a key the
/// observation does not have.
fn readings(game: &Game, conditions: &[Condition]) -> Vec<Option<Value>> {
    if conditions.is_empty() {
        return Vec::new();
    }
    let observed = crate::obs::observation_spectator(game, 0);
    let players = observed["players"].as_array();
    conditions
        .iter()
        .map(|condition| {
            players?
                .iter()
                .find(|player| player["id"] == json!(condition.seat))?
                .get(&condition.key)
                .cloned()
        })
        .collect()
}

/// What the conditions read of the world a step left.
///
/// A step holds each condition's reading before it against the reading after
/// it, and both are the omniscient observation of the whole table: at turn
/// 80 to 105 of a six-seat game the two cost a step 60 to 80 ms of its 150.
/// The reading after one step is the reading before the next whenever the
/// world between them is the same world and the conditions are the same
/// conditions, so it is kept and only then used.
#[derive(Clone)]
struct Read {
    /// The world it was read of. A game is its seed and its log, so a world
    /// with this log, action for action, is that world.
    log: crate::actionlog::ActionLog,
    turn: u32,
    conditions: Vec<Condition>,
    readings: Vec<Option<Value>>,
}

impl Read {
    /// These readings, if `game` is the world they were read of and
    /// `conditions` are what was read.
    fn of(self, game: &Game, conditions: &[Condition]) -> Option<Vec<Option<Value>>> {
        (self.turn == game.turn
            && self.log.len() == game.log.len()
            && game.log.continues(&self.log)
            && self.conditions == conditions)
            .then_some(self.readings)
    }
}

/// When a run stops by itself.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Breakpoints {
    /// Stop when one of these turns opens.
    turns: BTreeSet<u32>,
    /// Stop on an event of one of these categories (`EVENT_CATEGORIES`).
    categories: BTreeSet<String>,
    /// Stop on an event whose text holds one of these, compared in lowercase.
    phrases: Vec<String>,
    /// Stop on any event the engine marked important.
    important: bool,
    /// Read every civilization's events, not only the played seat's. The
    /// seat's own are what a person playing it is shown; a debugger may want
    /// the ones it would not be.
    every_seat: bool,
    /// Stop when one of these becomes true of the game's state.
    conditions: Vec<Condition>,
    /// Stop on the first turn of a new world era.
    eras: bool,
}

impl Breakpoints {
    fn from_request(request: &Value, game: &Game) -> Result<Breakpoints, String> {
        let mut conditions = Vec::new();
        for asked in request["conditions"].as_array().into_iter().flatten() {
            let seat = asked["seat"]
                .as_u64()
                .and_then(|seat| usize::try_from(seat).ok())
                .filter(|seat| *seat < game.players.len())
                .ok_or_else(|| "a condition names a civilization by its seat".to_string())?;
            let key = asked["key"].as_str().unwrap_or("").to_string();
            let sign = asked["test"].as_str().unwrap_or("");
            let test = Test::parse(sign).ok_or_else(|| {
                format!("a condition's test is one of < <= > >= == != changes, not {sign:?}")
            })?;
            let line = match (test, asked["value"].as_f64()) {
                (Test::Changes, _) => 0.0,
                (_, Some(line)) => line,
                (_, None) => return Err(format!("{key} {sign} needs a number to hold it to")),
            };
            conditions.push(Condition {
                seat,
                key,
                test,
                line,
            });
        }
        if conditions.len() > MAX_CONDITIONS {
            return Err(format!("at most {MAX_CONDITIONS} breakpoint conditions"));
        }
        // A condition is read back from where it lands before it is kept: a
        // key the observation does not have would never stop anything, and
        // would say nothing about why.
        for (condition, reading) in conditions.iter().zip(readings(game, &conditions)) {
            let readable = match (condition.test, &reading) {
                (_, None) => false,
                (Test::Changes, Some(_)) => true,
                (_, Some(value)) => as_number(value).is_some(),
            };
            if !readable {
                return Err(format!(
                    "seat {} has no {} called {:?}",
                    condition.seat,
                    if condition.test == Test::Changes {
                        "reading"
                    } else {
                        "number"
                    },
                    condition.key
                ));
            }
        }
        let mut turns = BTreeSet::new();
        for turn in request["turns"].as_array().into_iter().flatten() {
            let turn = turn
                .as_u64()
                .and_then(|turn| u32::try_from(turn).ok())
                .filter(|turn| *turn > 0)
                .ok_or_else(|| "a breakpoint turn is a positive number".to_string())?;
            turns.insert(turn);
        }
        if turns.len() > MAX_BREAK_TURNS {
            return Err(format!("at most {MAX_BREAK_TURNS} breakpoint turns"));
        }
        let mut categories = BTreeSet::new();
        for category in request["categories"].as_array().into_iter().flatten() {
            let name = category.as_str().unwrap_or("");
            if !crate::game::EVENT_CATEGORIES.contains(&name) {
                return Err(format!("no event category named {name:?}"));
            }
            categories.insert(name.to_string());
        }
        let mut phrases = Vec::new();
        for phrase in request["phrases"].as_array().into_iter().flatten() {
            let phrase = phrase.as_str().unwrap_or("").trim().to_lowercase();
            if !phrase.is_empty() && !phrases.contains(&phrase) {
                phrases.push(phrase);
            }
        }
        if phrases.len() > MAX_PHRASES {
            return Err(format!("at most {MAX_PHRASES} breakpoint phrases"));
        }
        Ok(Breakpoints {
            turns,
            categories,
            phrases,
            important: request["important"].as_bool().unwrap_or(false),
            every_seat: request["every_seat"].as_bool().unwrap_or(false),
            conditions,
            eras: request["eras"].as_bool().unwrap_or(false),
        })
    }

    /// The first condition a freshly played turn made true: one that holds
    /// now and did not a turn ago. A condition that stays true does not stop
    /// every turn it stays true for.
    fn crossed(
        &self,
        turn: u32,
        before: &[Option<Value>],
        after: &[Option<Value>],
    ) -> Option<Value> {
        self.conditions
            .iter()
            .zip(before.iter().zip(after))
            .find_map(|(condition, (was, now))| {
                let (was, now) = (was.as_ref()?, now.as_ref()?);
                let crossed = match condition.test {
                    Test::Changes => was != now,
                    test => {
                        let (was, now) = (as_number(was)?, as_number(now)?);
                        test.holds(now, condition.line) && !test.holds(was, condition.line)
                    }
                };
                crossed.then(|| {
                    // A number where the reading is one, so a list is
                    // reported as the length it was held to and not in full.
                    let said = |reading: &Value| {
                        as_number(reading).map_or_else(|| reading.clone(), |number| json!(number))
                    };
                    let mut hit = condition.json();
                    hit["kind"] = json!("condition");
                    hit["turn"] = json!(turn);
                    hit["was"] = said(was);
                    hit["now"] = said(now);
                    hit
                })
            })
    }

    fn json(&self) -> Value {
        json!({
            "turns": self.turns,
            "categories": self.categories,
            "phrases": self.phrases,
            "important": self.important,
            "every_seat": self.every_seat,
            "conditions": self.conditions.iter().map(Condition::json).collect::<Vec<_>>(),
            "eras": self.eras,
            // The vocabulary a page offers, read from the engine's own lists.
            "known_categories": crate::game::EVENT_CATEGORIES,
            "known_tests": Test::ALL.map(Test::sign),
        })
    }

    /// The first thing in a freshly played turn that a breakpoint names.
    fn hit(&self, game: &Game, fresh: &[Event]) -> Option<Value> {
        if self.turns.contains(&game.turn) {
            return Some(json!({"kind": "turn", "turn": game.turn}));
        }
        fresh
            .iter()
            .filter(|event| self.every_seat || event.player == 0)
            .find(|event| {
                (self.important && event.important)
                    || self.categories.contains(&event.category)
                    || (!self.phrases.is_empty() && {
                        let text = event.text.to_lowercase();
                        self.phrases.iter().any(|phrase| text.contains(phrase))
                    })
            })
            .map(|event| {
                json!({
                    "kind": "event",
                    "turn": event.turn,
                    "player": event.player,
                    "category": event.category,
                    "text": event.text,
                    "pos": event.pos,
                })
            })
    }
}

/// Marks a timeline holds before the oldest are thinned. A mark costs what
/// its turn changed, which was 1.3 MB early and 2.8 MB by turn 130 in a
/// six-seat game when this was measured, so this many is about the ceiling of
/// what a long game should be allowed to keep.
const ROOMY_MARKS: usize = 480;
/// The coarsest the older turns are thinned to before the oldest simply go.
const COARSEST_STRIDE: u32 = 64;
/// The fewest marks a timeline is squeezed to.
const MIN_MARKS: usize = 12;
/// Memory in use past which the marks start giving room back, unless the
/// page names another line (`room`).
const MEMORY_LINE: usize = 1 << 30;
/// Past the line, each further growth by the line over this many takes room
/// away again: 32 MB at 1 GB.
const MEMORY_STEPS: usize = 32;
/// Save points a timeline holds at once.
const MAX_SAVE_POINTS: usize = 32;
/// The longest name a save point is given.
const MAX_SAVE_NAME: usize = 48;
/// The save point that going to another one leaves behind.
const WHERE_YOU_WERE: &str = "Where you were";

/// The memory this process can see itself using, where it can: the browser
/// build's linear memory, which only grows and has a ceiling.
#[cfg(target_arch = "wasm32")]
fn memory_in_use() -> Option<usize> {
    Some(core::arch::wasm32::memory_size(0).saturating_mul(65_536))
}

/// A native process is not told what it holds, and is not near a ceiling.
#[cfg(not(target_arch = "wasm32"))]
fn memory_in_use() -> Option<usize> {
    None
}

/// Whether one of two worlds was played on from the other.
fn on_one_line(a: &Game, b: &Game) -> bool {
    a.log.continues(&b.log) || b.log.continues(&a.log)
}

/// A running FNV-1a of what is written to it.
struct Print(u64);

impl Print {
    const START: u64 = 0xCBF2_9CE4_8422_2325;
    const PRIME: u64 = 0x0000_0100_0000_01B3;
}

impl std::fmt::Write for Print {
    fn write_str(&mut self, text: &str) -> std::fmt::Result {
        for byte in text.bytes() {
            self.0 = (self.0 ^ u64::from(byte)).wrapping_mul(Self::PRIME);
        }
        Ok(())
    }
}

/// A print of the world that has just been marked: a number two marks share
/// only when they hold the same world.
///
/// Two sessions that began as one world and were then each played by
/// themselves, as the halves of a split are, hold the same world for as long
/// as the same actions were applied to both: the engine is deterministic and
/// the random stream is part of the game. So the print is the print of the
/// mark this world was played on from, carried on over every action applied
/// since, by whichever seat. It then takes in a few things read off the world
/// itself, the turn, the random stream and what every seat has in its
/// treasury and towards its research and its civic, which tell apart two
/// worlds that differ by something no action carries (how a city was told to
/// work its tiles).
///
/// `earlier` is the newest mark before this one. A world that was not played
/// on from it, the first of a timeline, starts a print of its own: the two
/// halves of a split share that mark as they share every turn behind the
/// split, by having been copied from one session.
fn world_print(game: &Game, earlier: Option<&Mark>) -> u64 {
    use std::fmt::Write;
    let (mut print, since) = match earlier.filter(|mark| game.log.continues(&mark.game.log)) {
        Some(mark) => (Print(mark.world), mark.actions),
        None => (Print(Print::START), game.log.len()),
    };
    // Writing to a print cannot fail.
    for (seat, action) in game.log.since(since) {
        let _ = write!(print, "{seat}{action:?};");
    }
    let _ = write!(print, "{}|{}|{:?}", game.turn, game.log.len(), game.rng);
    for player in &game.players {
        let held = [player.gold, player.research_progress, player.civic_progress];
        for number in held {
            let _ = write!(print, "|{}", number.to_bits());
        }
    }
    print.0
}

/// A print as the page is given it: the bits a number in a browser keeps.
const PRINT_SHIFT: u32 = 12;

/// The world as it stood when the played seat's turn opened.
#[derive(Clone)]
struct Mark {
    turn: u32,
    /// How long the action log was. Everything past it in the live game is
    /// what the seat has done with the turn since.
    actions: usize,
    game: Game,
    /// The Civ AI whose play opened this turn, by its id: it held the seat
    /// for the turn before. `None` for a turn the person ended by hand, and
    /// for the turn a timeline begins on.
    by: Option<String>,
    /// Whether that agent played the turn before from its top to its end,
    /// with nothing done in it first by hand or by a layer. Only such a turn
    /// is what the agent plays from the mark before it, so only such a turn
    /// is taken from its mark instead of being played again (`debug_walk`).
    whole: bool,
    /// The agents as they sat when the turn opened. `None` in a half of a
    /// split, which keeps none.
    minds: Option<Minds>,
    /// The print of this world (`world_print`).
    world: u64,
}

/// One seat's agent as it was kept, or `None` for one that could not be
/// copied (`Ai::fork`).
type Seated = Option<Box<dyn Ai + Send>>;

/// The agents at one moment, each a copy with what it remembered then.
///
/// An agent keeps plans from turn to turn, so the world alone is not all of
/// a game: the same world played on by an agent that has just sat down is
/// another game within a turn or two (measured, `docs/SIM_DEBUGGER.md`).
/// Kept with a mark or a save point, these are who plays on when that world
/// is put back, so a turn played again from there is the turn that was
/// played.
///
/// A copy is taken out through the lock and the kept ones are never played,
/// so every clone of a mark may share them.
#[derive(Clone)]
struct Minds {
    seats: Arc<Mutex<Vec<Seated>>>,
    /// The Civ AI seated in the played seat then, by its id.
    seat: Option<String>,
    /// The Civ AI the rivals were seated with then, if one had been chosen.
    rivals: Option<String>,
}

/// The rest of a turn the seat's agent planned, good for as long as the world
/// is where the plan left it.
#[derive(Clone)]
struct Plan {
    turn: u32,
    /// How long the action log was when the plan was made or last advanced.
    /// Anything else that touched the world since makes the plan stale.
    at: usize,
    rest: VecDeque<Action>,
    /// Whether the plan was made at the top of the turn, before anything was
    /// done in it (`Mark::whole`).
    whole: bool,
    /// The world as the agent left it: the copy it played the turn on,
    /// before the turn is ended. The last step puts this on the table, so a
    /// turn taken apart ends on exactly the world the turn taken whole does.
    done: Game,
}

/// A world kept under a name: somewhere to come back to however the game is
/// played on from here. It is the world as it stood, part-way through a turn
/// or not, and the top of that turn to undo back to.
#[derive(Clone)]
struct SavePoint {
    id: u32,
    name: String,
    /// Kept by the debugger itself, when a world was left for a save point.
    auto: bool,
    turn: u32,
    /// What the seat had done with the turn.
    acted: usize,
    game: Game,
    /// The turn's own mark, which the timeline may since have replaced.
    top: Option<Mark>,
    /// The agents as they sat when it was set (`Minds`). `None` in a half of
    /// a split.
    minds: Option<Minds>,
}

/// A played game's marks, and what the debugger was told to do with it.
///
/// Cloning one is cheap for the reason keeping one is: a mark shares with its
/// clone everything the game shares with its own.
#[derive(Clone, Default)]
pub struct Timeline {
    /// Oldest first, one a turn at most.
    marks: Vec<Mark>,
    breakpoints: Breakpoints,
    layers: BTreeSet<Layer>,
    /// Why the last step stopped a run, until the next thing moves the world.
    stopped: Option<Value>,
    /// What the conditions read after the last step (`Read`). Any other
    /// debugger operation lets go of it.
    read: Option<Read>,
    /// What the layers did when the open turn began.
    assisted: usize,
    /// The agent's turn, when it is being taken an action at a time.
    plan: Option<Plan>,
    /// What the last single step did, until the next thing moves the world.
    stepped: Option<Value>,
    /// Show the page the whole world rather than what the seat can see.
    reveal: bool,
    /// Worlds kept under a name.
    saves: Vec<SavePoint>,
    /// The id the last save point was given.
    save_ids: u32,
    /// How many marks there is room for, once memory has had a say.
    room: Option<usize>,
    /// Where the page put the memory line, if it did.
    memory_line: Option<usize>,
    /// The memory reading that last took room away.
    squeezed_at: usize,
    /// A half of a split keeps no agents with its marks and seats none from
    /// them (`Session::fork`): a walk in a half seats new agents, as it
    /// always has, so the two halves of a head to head differ only in the
    /// Civ AI each seats.
    forgetful: bool,
    /// What a search playing this world is asked for (`op: "objective"`,
    /// `autosearch.rs`): each turn a `play` plays is read for its terms, and
    /// a Must Avoid broken for good ends the play there.
    objective: Option<super::autosearch::Objective>,
}

impl Timeline {
    /// Start over on this world: a new game, or a loaded one.
    fn begin(&mut self, game: &Game) {
        self.marks.clear();
        self.saves.clear();
        self.stopped = None;
        self.assisted = 0;
        // What an earlier game grew the memory to says nothing about this
        // one; only growing past it does.
        self.room = None;
        self.squeezed_at = memory_in_use().unwrap_or(0);
        self.record(game);
    }

    /// Mark the turn that just opened. Whatever was marked from this turn on
    /// belongs to a future this world did not take.
    fn record(&mut self, game: &Game) {
        if !Self::markable(game) {
            return;
        }
        self.marks.retain(|mark| mark.turn < game.turn);
        let world = world_print(game, self.marks.last());
        self.marks.push(Mark {
            turn: game.turn,
            actions: game.log.len(),
            game: game.clone(),
            by: None,
            whole: false,
            minds: None,
            world,
        });
        self.thin();
    }

    /// The marks that are the seat's own line under the Civ AI seated there:
    /// every turn up to the open one, and the turns ahead for as long as that
    /// agent is the one that played them.
    ///
    /// A turn ahead that another Civ AI played is that agent's game, not what
    /// this one would do from here. It is not walked to and not counted among
    /// the turns played; it stays kept, and is this line again if its agent
    /// is seated again before the turns are played over. A turn the person
    /// ended by hand is nobody else's, and neither is any turn while no agent
    /// has been named for the seat.
    fn line(&self, turn: u32, seat: Option<&str>) -> &[Mark] {
        let mut held = self.marks.partition_point(|mark| mark.turn <= turn);
        while let Some(mark) = self.marks.get(held) {
            if matches!((mark.by.as_deref(), seat), (Some(by), Some(seat)) if by != seat) {
                break;
            }
            held += 1;
        }
        &self.marks[..held]
    }

    /// Only the top of the played seat's own turn is a place to come back to.
    fn markable(game: &Game) -> bool {
        game.current == 0 && game.is_human_seat(0)
    }

    /// How many marks may be kept.
    fn room(&self) -> usize {
        self.room.unwrap_or(ROOMY_MARKS).max(MIN_MARKS)
    }

    /// Hear how much memory is in use. Past the line, the room becomes a
    /// fifth less than what is held, and again each time the memory has grown
    /// by another step: memory that only grows cannot say that it was given
    /// back, so the growing is what is listened to.
    ///
    /// It goes on down to `MIN_MARKS`, well below the newest `DENSE_MARKS`.
    /// A floor that kept those whole did not hold the memory: late marks are
    /// larger than the early ones they replace, so the same number of marks
    /// kept growing (measured, `docs/SIM_DEBUGGER.md`). Few turns to go back
    /// to are worth more than a tab that has run out of memory.
    fn squeeze(&mut self, in_use: usize) {
        let line = self.memory_line.unwrap_or(MEMORY_LINE);
        let step = line / MEMORY_STEPS;
        if in_use <= line || in_use < self.squeezed_at.saturating_add(step) {
            return;
        }
        self.squeezed_at = in_use;
        let held = self.marks.len().min(self.room());
        self.room = Some((held - held / 5).max(MIN_MARKS));
    }

    /// Make room when there are more marks than there is room for. The room
    /// is never exceeded.
    ///
    /// The first mark always stays, and so do the newest `DENSE_MARKS`, or
    /// the newest half of the room when that is fewer. Between them the
    /// oldest turns go first and the odd ones before the even, so a long
    /// game's early turns are kept one in two, then one in four, and so on to
    /// one in `COARSEST_STRIDE`. If that is still too many, the oldest go.
    fn thin(&mut self) {
        if let Some(in_use) = memory_in_use() {
            self.squeeze(in_use);
        }
        let room = self.room();
        let dense = DENSE_MARKS.min(room / 2);
        let mut over = self.marks.len().saturating_sub(room);
        let mut stride = 2;
        while over > 0 {
            let old = self.marks.len().saturating_sub(dense);
            let last_pass = stride > COARSEST_STRIDE;
            let mut index = 0;
            self.marks.retain(|mark| {
                let kept_by_stride = !last_pass && mark.turn % stride == 0;
                let goes = over > 0 && index > 0 && index < old && !kept_by_stride;
                index += 1;
                if goes {
                    over -= 1;
                }
                !goes
            });
            if last_pass {
                break;
            }
            stride *= 2;
        }
    }

    /// Keep a world under a name. A name already taken is given to this
    /// world instead, and the debugger's own save point is one of a kind.
    fn keep(&mut self, name: &str, game: &Game, auto: bool) -> Result<u32, String> {
        self.saves
            .retain(|save| save.name != name && !(auto && save.auto));
        if self.saves.len() >= MAX_SAVE_POINTS {
            return Err(format!(
                "{MAX_SAVE_POINTS} save points are kept at once; remove one first"
            ));
        }
        let acted = self.acted(game);
        let top = self
            .mark(game.turn)
            .filter(|mark| game.log.continues(&mark.game.log))
            .cloned();
        self.save_ids += 1;
        self.saves.push(SavePoint {
            id: self.save_ids,
            name: name.to_string(),
            auto,
            turn: game.turn,
            acted,
            game: game.clone(),
            top,
            minds: None,
        });
        Ok(self.save_ids)
    }

    fn mark(&self, turn: u32) -> Option<&Mark> {
        self.marks.iter().find(|mark| mark.turn == turn)
    }

    /// The nearest marked turn at or before `turn`.
    fn mark_at_or_before(&self, turn: u32) -> Option<&Mark> {
        self.marks.iter().rev().find(|mark| mark.turn <= turn)
    }

    /// What the seat has done since the open turn's mark.
    fn acted(&self, game: &Game) -> usize {
        self.mark(game.turn)
            .map_or(0, |mark| game.log.len().saturating_sub(mark.actions))
    }

    /// The first kept turn of each world era the marks pass through: where
    /// the page marks an era's start on the timeline, and a turn that can be
    /// gone back to. When the turn an era began on has been thinned away it
    /// is the nearest kept turn after it.
    fn eras(marks: &[Mark]) -> Vec<(usize, u32)> {
        let mut eras: Vec<(usize, u32)> = Vec::new();
        for mark in marks {
            if eras
                .last()
                .is_none_or(|(era, _)| *era != mark.game.world_era)
            {
                eras.push((mark.game.world_era, mark.turn));
            }
        }
        eras
    }

    /// Who played each stretch of the kept turns: from which kept turn, to
    /// which, and the Civ AI whose play it was (`Mark::by`; `None` for turns
    /// the person ended by hand). A stretch begins on the turn its first
    /// turn was played from, so each begins where the one before it ends,
    /// and a timeline that changes hands says on which turn it did.
    ///
    /// These are all the marks and not only the seat's line (`line`): the
    /// turns ahead that another Civ AI played are kept, and the page says
    /// whose they are as well.
    fn played_by(&self) -> Vec<(u32, u32, Option<&str>)> {
        let mut stretches: Vec<(u32, u32, Option<&str>)> = Vec::new();
        for pair in self.marks.windows(2) {
            let (from, mark) = (pair[0].turn, &pair[1]);
            match stretches.last_mut() {
                Some((_, to, by)) if *by == mark.by.as_deref() => *to = mark.turn,
                _ => stretches.push((from, mark.turn, mark.by.as_deref())),
            }
        }
        stretches
    }

    /// Let go of every turn kept ahead of `turn`: the world there is the
    /// start of a branch of its own, and what was played past it stays with
    /// the timeline it was branched from.
    fn branch(&mut self, turn: u32) {
        self.marks.retain(|mark| mark.turn <= turn);
    }

    /// `seat` is the Civ AI seated at the played seat, when one has been
    /// named: the turns given are its line (`line`).
    fn json(&self, game: &Game, seat: Option<&str>) -> Value {
        let line = self.line(game.turn, seat);
        let turns: Vec<u32> = line.iter().map(|mark| mark.turn).collect();
        let acted = self.acted(game);
        json!({
            "turn": game.turn,
            "turns": turns,
            // The print of each of those turns' worlds, in the same order:
            // two timelines hold the same world on a turn they both give the
            // same print for (`world_print`).
            "worlds": line.iter().map(|mark| mark.world >> PRINT_SHIFT).collect::<Vec<_>>(),
            "first": turns.first(),
            "last": turns.last(),
            // The Civ AI seated, and the kept turns ahead that another one
            // played: from which turn, to which, and who played the first.
            "seat": seat,
            "beyond": self.marks.get(line.len()).map(|mark| json!({
                "from": mark.turn,
                "to": self.marks.last().map(|last| last.turn),
                "by": mark.by,
            })),
            "acted": acted,
            // Whose turn it is. Anything but the played seat is the middle of
            // a round being stepped a seat at a time.
            "playing": game.current,
            "can_undo": acted > 0 && game.current == 0 && self.mark(game.turn).is_some(),
            "can_back": acted > 0 || turns.first().is_some_and(|first| *first < game.turn),
            "can_forward": turns.last().is_some_and(|last| *last > game.turn),
            "eras": Self::eras(line).iter().map(|(era, turn)| json!({
                "era": era,
                "name": crate::rules::ERA_NAMES.get(*era),
                "turn": turn,
            })).collect::<Vec<_>>(),
            // Whose play each stretch of the kept turns was, the ones ahead
            // that are not on the line too, for the page to name on the
            // timeline (`played_by`).
            "played_by": self.played_by().iter().map(|(from, to, by)| json!({
                "from": from,
                "to": to,
                "by": by,
            })).collect::<Vec<_>>(),
            "breakpoints": self.breakpoints.json(),
            "layers": Layer::ALL.iter().map(|layer| json!({
                "id": layer.id(),
                "label": layer.label(),
                "on": self.layers.contains(layer),
            })).collect::<Vec<_>>(),
            "assisted": self.assisted,
            "stopped": self.stopped,
            "stepped": self.stepped,
            "reveal": self.reveal,
            "memory_line": self.memory_line.map(|bytes| bytes >> 20),
            "saves": self.saves.iter().map(|save| json!({
                "id": save.id,
                "name": save.name,
                "auto": save.auto,
                "turn": save.turn,
                "acted": save.acted,
            })).collect::<Vec<_>>(),
        })
    }
}

/// The events a step added to a log that is trimmed from its front.
fn fresh_events<'a>(
    before_len: usize,
    before_last: Option<&Event>,
    turn: u32,
    now: &'a [Event],
) -> &'a [Event] {
    let same = |a: &Event, b: &Event| a.turn == b.turn && a.player == b.player && a.text == b.text;
    let Some(last) = before_last else {
        return now;
    };
    if now.len() >= before_len && before_len > 0 && same(&now[before_len - 1], last) {
        return &now[before_len..];
    }
    // The log was trimmed while the turn played. Find where the old tail went.
    if let Some(at) = now.iter().rposition(|event| same(event, last)) {
        return &now[at + 1..];
    }
    let from = now.partition_point(|event| event.turn < turn);
    &now[from..]
}

impl Session {
    /// Whether this is a game the debugger can walk: one with a seat of its
    /// own. A watched table plays itself and keeps no marks.
    fn debuggable(&self) -> bool {
        !self.params.spectate && self.mirror.is_none() && self.game.is_human_seat(0)
    }

    /// Start the timeline on the world this session was just given.
    pub(super) fn debugger_begins(&mut self) {
        if self.debuggable() {
            self.debugger.begin(&self.game);
            self.debug_keep_minds();
        }
    }

    /// The played seat's next turn has opened by being played into.
    ///
    /// `assist` is whether the person is the one about to play it: a turn an
    /// agent is playing whole has no use for the layers.
    pub(super) fn debugger_turn_opened(&mut self, assist: bool) {
        if !self.debuggable() {
            return;
        }
        self.debugger.stopped = None;
        self.debugger.stepped = None;
        self.debugger.plan = None;
        self.debugger.assisted = 0;
        self.debugger.record(&self.game);
        self.debug_keep_minds();
        if assist && !self.debugger.layers.is_empty() {
            self.debugger.assisted = self.debug_assist();
        }
    }

    /// Whether the open turn stands as it opened: it is the seat's to play,
    /// it has its mark, and nothing has been done in it since.
    pub(super) fn debugger_turn_stands(&self) -> bool {
        let game = &self.game;
        game.current == 0
            && self.debugger.mark(game.turn).is_some_and(|mark| {
                mark.actions == game.log.len() && game.log.continues(&mark.game.log)
            })
    }

    /// The seat's agent played the turn that was open on `from`. If that
    /// opened the seat's next turn, the turn is marked as that agent's work
    /// (`Mark::by`), which is what says whose game the turns ahead are once
    /// the world is walked back. `whole` is whether the turn stood as it
    /// opened when the agent took it (`debugger_turn_stands`, `Mark::whole`).
    pub(super) fn debugger_agent_played(&mut self, from: u32, whole: bool) {
        let turn = self.game.turn;
        if !self.debuggable() || turn <= from || !Timeline::markable(&self.game) {
            return;
        }
        let by = self.autoplay_strategy.clone();
        if let Some(mark) = self.debugger.marks.last_mut() {
            if mark.turn == turn {
                mark.by = by;
                mark.whole = whole;
            }
        }
    }

    /// The agents as they sit now, each a copy with what it remembers.
    fn debug_minds(&self) -> Minds {
        Minds {
            seats: Arc::new(Mutex::new(self.ais.iter().map(|ai| ai.fork()).collect())),
            seat: self.autoplay_strategy.clone(),
            rivals: self.rival_strategy.clone(),
        }
    }

    /// Keep the agents as they sit with the mark of the turn that just
    /// opened. A half of a split keeps none (`Timeline::forgetful`).
    fn debug_keep_minds(&mut self) {
        let turn = self.game.turn;
        let wanted = matches!(self.debugger.marks.last(), Some(mark) if mark.turn == turn && mark.minds.is_none());
        if self.debugger.forgetful || !wanted {
            return;
        }
        let minds = self.debug_minds();
        if let Some(mark) = self.debugger.marks.last_mut() {
            mark.minds = Some(minds);
        }
    }

    /// Keep the agents as they sit with the save point just set.
    fn debug_mind_save(&mut self) {
        if self.debugger.forgetful {
            return;
        }
        let minds = self.debug_minds();
        if let Some(save) = self.debugger.saves.last_mut() {
            save.minds = Some(minds);
        }
    }

    /// Seat the agents kept with the world that was just put on the table.
    ///
    /// The played seat's is taken only while the Civ AI seated there is the
    /// one that was seated then: one picked since plays from here as itself.
    /// The rivals' are taken only while they are seated with what they were
    /// seated with then. A seat whose agent could not be copied keeps the
    /// new one it was just given.
    fn debug_seat_minds(&mut self, minds: &Minds) {
        let Ok(kept) = minds.seats.lock() else {
            return;
        };
        let rivals = minds.rivals == self.rival_strategy;
        for (seat, mind) in kept.iter().enumerate().take(self.ais.len()) {
            let held = if seat == 0 {
                minds.seat == self.autoplay_strategy
            } else {
                rivals
            };
            let Some(mind) = mind.as_ref().filter(|_| held).and_then(|mind| mind.fork()) else {
                continue;
            };
            self.ais[seat] = mind;
            self.ais[seat].attach_journal(self.journal.handle());
        }
        // A mind kept before the table's fog setting was changed plays on
        // under the setting the table has now.
        self.seat_fog();
    }

    /// A half that is kept becomes the game itself (`Panes::adopt`), and from
    /// then on keeps its agents with its marks as any game does.
    pub(super) fn debug_remembers(&mut self) {
        self.debugger.forgetful = false;
    }

    /// A half of a split is about to be searched: it keeps its agents with
    /// every turn it marks from here on, and none with the turns it has
    /// marked already.
    ///
    /// A half keeps none, so a turn it walks back to is played on by agents
    /// that have just sat down, which is another game than the one that was
    /// played on from it (`Minds`). A search plays one line, walks back to
    /// the middle of it and plays another from there, on a copy of the engine
    /// made for it (`docs/SIM_DEBUGGER.md`, "Autosearch"), and that has to be
    /// the second line played straight through. So the copy's half is asked
    /// this first, and from then on a turn it walks back to seats the agents
    /// that were playing it.
    ///
    /// The marks it holds already are left with no agents, some of them kept
    /// before the screen was split. That is how the half on the page has
    /// them: walked back to the turn the search begins on, the half and its
    /// copy both seat new agents there and play the same game from it, so
    /// what the search found is what the half plays. The game itself keeps
    /// its agents already and is asked nothing.
    fn debug_remember(&mut self) {
        if !self.debugger.forgetful {
            return;
        }
        for mark in &mut self.debugger.marks {
            mark.minds = None;
        }
        self.debugger.forgetful = false;
    }

    /// The marks on the seat's line as it stands (`Timeline::line`).
    fn debug_line(&self) -> &[Mark] {
        self.debugger
            .line(self.game.turn, self.autoplay_strategy.as_deref())
    }

    /// Whether the page is shown the world with the fog lifted.
    pub(super) fn debugger_reveals(&self) -> bool {
        self.debugger.reveal
    }

    /// The debugger's part of the state document.
    pub(super) fn debugger_json(&self) -> Value {
        let mut out = self
            .debugger
            .json(&self.game, self.autoplay_strategy.as_deref());
        // The last turn ahead that playing on from here would only play
        // again, which a run walks to instead (`debug_walk`).
        out["played_ahead"] = json!(self
            .debug_played_ahead()
            .map(|(_, last)| self.debugger.marks[last].turn));
        out
    }

    /// Put a marked world on the table in place of the live one.
    ///
    /// The agents sitting now are not the ones to play it: their plans belong
    /// to the future this world is being taken out of. The agents kept with
    /// the world (`Minds`) sit down again, so it is played on as it was.
    /// Where none were kept the agents are rebuilt, as they are for a loaded
    /// save.
    fn debug_put(&mut self, game: Game, minds: Option<Minds>) {
        let seat = self.autoplay_strategy.clone();
        let rivals = self.rival_strategy.clone();
        let view = self.view_player;
        let humans = std::mem::take(&mut self.human_players);
        let debugger = std::mem::take(&mut self.debugger);
        let resumed = self.resumed_next_game_params.take();
        let (rival_yields, own_yields) = (self.keeps_rival_yields, self.keeps_own_yields);
        let mut next = Session::from_saved_game(self.params.clone(), game);
        // The world gone back to is this table's: it keeps the table's Fog-honest
        // AI setting, which every agent seated below takes.
        next.set_fog_honest(self.fog_honest);
        if let Some(name) = rivals {
            let _ = next.seat_rivals(&name);
        }
        if let Some(name) = seat {
            let _ = next.seat_strategy_at(0, &name);
        }
        if let Some(minds) = minds.filter(|_| !debugger.forgetful) {
            next.debug_seat_minds(&minds);
        }
        if !humans.is_empty() {
            next.human_players = humans;
        }
        next.view_player = view;
        next.debugger = debugger;
        next.resumed_next_game_params = resumed;
        next.keeps_rival_yields = rival_yields;
        next.keeps_own_yields = own_yields;
        *self = next;
        self.debugger.stopped = None;
        self.debugger.stepped = None;
        self.debugger.plan = None;
        self.debugger.assisted = 0;
    }

    /// A second session on this one's world as it stands: the same seats and
    /// agents, and the same turns behind it to walk back along. From here the
    /// two play their own histories. This is what one half of a split screen
    /// is given (`panes.rs`).
    pub(super) fn fork(&self) -> Session {
        let mut next = Session::from_saved_game(self.params.clone(), self.game.clone());
        next.set_fog_honest(self.fog_honest);
        if let Some(name) = &self.rival_strategy {
            let _ = next.seat_rivals(name);
        }
        if let Some(name) = &self.autoplay_strategy {
            let _ = next.seat_strategy_at(0, name);
        }
        if !self.human_players.is_empty() {
            next.human_players = self.human_players.clone();
        }
        next.view_player = self.view_player;
        next.debugger = self.debugger.clone();
        next.debugger.forgetful = true;
        next.debugger.stopped = None;
        next.keeps_rival_yields = self.keeps_rival_yields;
        next.keeps_own_yields = self.keeps_own_yields;
        next
    }

    /// Go to a marked turn. A turn with no mark of its own answers with the
    /// nearest one before it. A turn ahead that another Civ AI played is not
    /// on this seat's line (`Timeline::line`) and is refused.
    pub fn debug_jump(&mut self, turn: u32) -> Result<u32, String> {
        let line = self.debug_line();
        if let Some(other) = self.debugger.marks.get(line.len()) {
            if other.turn <= turn {
                return Err(format!(
                    "turn {} was played by another Civ AI; the one seated plays on from here",
                    other.turn
                ));
            }
        }
        let Some(mark) = line.iter().rev().find(|mark| mark.turn <= turn) else {
            return Err(format!("turn {turn} is before this timeline begins"));
        };
        let (reached, game, minds) = (mark.turn, mark.game.clone(), mark.minds.clone());
        self.debug_put(game, minds);
        Ok(reached)
    }

    /// Step back: to the top of the open turn if the seat has acted in it,
    /// and otherwise to the marked turn before.
    pub fn debug_back(&mut self) -> Result<u32, String> {
        let turn = self.game.turn;
        if self.debugger.acted(&self.game) > 0 {
            return self.debug_jump(turn);
        }
        let Some(before) = turn
            .checked_sub(1)
            .filter(|before| self.debugger.mark_at_or_before(*before).is_some())
        else {
            return Err("this is where the timeline begins".to_string());
        };
        self.debug_jump(before)
    }

    /// Step forward along turns already played, without playing anything.
    /// Only the seat's own line is walked: a turn ahead that another Civ AI
    /// played is refused (`debug_jump`).
    pub fn debug_forward(&mut self) -> Result<u32, String> {
        let turn = self.game.turn;
        let Some(next) = self
            .debugger
            .marks
            .iter()
            .find(|mark| mark.turn > turn)
            .map(|mark| mark.turn)
        else {
            return Err("nothing has been played past this turn".to_string());
        };
        self.debug_jump(next)
    }

    /// The kept turns ahead that playing on from here would only play again,
    /// as the places of the open turn's mark and of the last of them.
    ///
    /// A turn is one of them where its mark is what the seated Civ AI makes
    /// of the turn before: the open turn stands as it opened, with the
    /// agents kept there to sit down again (`Minds`; a half of a split keeps
    /// none) and the rivals seated as they were, and every turn from here to
    /// there was that agent's from top to end (`Mark::by`, `Mark::whole`),
    /// with no turn between thinned away. A turn the person ended, touched
    /// first, or another Civ AI played is where they end: from there the
    /// seated agent's play is another game.
    fn debug_played_ahead(&self) -> Option<(usize, usize)> {
        let seat = self.autoplay_strategy.as_deref()?;
        let marks = &self.debugger.marks;
        let at = marks.partition_point(|mark| mark.turn < self.game.turn);
        let here = marks.get(at)?;
        let stands = here.turn == self.game.turn
            && !self.debugger.forgetful
            && self.debugger_turn_stands()
            && here
                .minds
                .as_ref()
                .is_some_and(|minds| minds.rivals == self.rival_strategy);
        if !stands {
            return None;
        }
        let mut last = at;
        while let Some(next) = marks.get(last + 1) {
            let played_again =
                next.whole && next.by.as_deref() == Some(seat) && next.turn == marks[last].turn + 1;
            if !played_again {
                break;
            }
            last += 1;
        }
        (last > at).then_some((at, last))
    }

    /// Go on along the turns the seated Civ AI has already played from this
    /// world, to `until` at most, without playing any of them again: the
    /// latest kept turn on the way is put on the table, and a run plays on
    /// from there (`debug_played_ahead` says which turns those are).
    ///
    /// The breakpoints are asked of every turn passed, each against the turn
    /// before it as when it was played, so the walk stops where the run that
    /// played them would have. `era` stops it on the first turn of a new
    /// world era as well, for a run that plays until one begins. The answer
    /// is the turn reached: the open one when there was nothing to take.
    pub fn debug_walk(&mut self, until: Option<u32>, era: bool) -> u32 {
        let turn = self.game.turn;
        let Some((at, last)) = self.debug_played_ahead() else {
            return turn;
        };
        let marks = &self.debugger.marks;
        let breakpoints = &self.debugger.breakpoints;
        let mut read = readings(&self.game, &breakpoints.conditions);
        let mut reached = at;
        let mut stopped = None;
        for next in at + 1..=last {
            let (before, mark) = (&marks[reached].game, &marks[next]);
            if until.is_some_and(|until| mark.turn > until) {
                break;
            }
            let game = &mark.game;
            let fresh = fresh_events(
                before.chronicle.events.len(),
                before.chronicle.events.last(),
                before.turn,
                &game.chronicle.events,
            );
            stopped = if game.is_finished() || !game.players[0].alive {
                Some(json!({"kind": "end", "turn": game.turn}))
            } else {
                breakpoints
                    .hit(game, fresh)
                    .or_else(|| {
                        ((era || breakpoints.eras) && game.world_era != before.world_era).then(
                            || json!({"kind": "era", "turn": game.turn, "era": game.world_era}),
                        )
                    })
                    .or_else(|| {
                        let after = readings(game, &breakpoints.conditions);
                        let hit = breakpoints.crossed(game.turn, &read, &after);
                        read = after;
                        hit
                    })
            };
            reached = next;
            if stopped.is_some() {
                break;
            }
        }
        if reached == at {
            return turn;
        }
        let mark = &marks[reached];
        let (to, game, minds) = (mark.turn, mark.game.clone(), mark.minds.clone());
        self.debug_put(game, minds);
        self.debugger.stopped = stopped;
        to
    }

    /// Keep the world as it stands under a name, to come back to.
    pub fn debug_save_point(&mut self, name: &str) -> Result<u32, String> {
        let name: String = name.trim().chars().take(MAX_SAVE_NAME).collect();
        if name.is_empty() {
            let turn = self.game.turn;
            return self
                .debugger
                .keep(&format!("Turn {turn}"), &self.game, false);
        }
        let kept = self.debugger.keep(&name, &self.game, false);
        if kept.is_ok() {
            self.debug_mind_save();
        }
        kept
    }

    /// Go to a save point: its world, with the turns of its own history.
    ///
    /// Marks of a line the save point is not on are let go, because a
    /// timeline is one history. The world being left is kept as a save point
    /// of the debugger's own, unless it is one already, so the way back is
    /// one more of these.
    pub fn debug_goto_save(&mut self, id: u32) -> Result<u32, String> {
        let Some(save) = self.debugger.saves.iter().find(|save| save.id == id) else {
            return Err("no save point has that id".to_string());
        };
        let (game, top, minds) = (save.game.clone(), save.top.clone(), save.minds.clone());
        let left = &self.game;
        let kept = self
            .debugger
            .saves
            .iter()
            .any(|save| save.game.log.len() == left.log.len() && on_one_line(&save.game, left));
        if !kept {
            // With no room for it the world left is simply not kept.
            if self.debugger.keep(WHERE_YOU_WERE, &self.game, true).is_ok() {
                self.debug_mind_save();
            }
        }
        self.debugger
            .marks
            .retain(|mark| on_one_line(&mark.game, &game));
        if let Some(top) = top.filter(|top| self.debugger.mark(top.turn).is_none()) {
            let at = self
                .debugger
                .marks
                .partition_point(|mark| mark.turn < top.turn);
            self.debugger.marks.insert(at, top);
        }
        let turn = game.turn;
        self.debug_put(game, minds);
        Ok(turn)
    }

    /// Let a save point go.
    pub fn debug_drop_save(&mut self, id: u32) -> Result<(), String> {
        let before = self.debugger.saves.len();
        self.debugger.saves.retain(|save| save.id != id);
        if self.debugger.saves.len() == before {
            return Err("no save point has that id".to_string());
        }
        Ok(())
    }

    /// Take back the seat's last action in the open turn.
    pub fn debug_undo(&mut self) -> Result<usize, String> {
        let turn = self.game.turn;
        let Some(mark) = self.debugger.mark(turn) else {
            return Err("this turn has no mark to undo back to".to_string());
        };
        let done: Vec<(usize, Action)> = self.game.log.since(mark.actions).cloned().collect();
        if done.is_empty() {
            return Err("nothing has been done this turn".to_string());
        }
        let (mut game, minds) = (mark.game.clone(), mark.minds.clone());
        for (seat, action) in &done[..done.len() - 1] {
            game.apply(*seat, action)
                .map_err(|error| format!("the turn could not be replayed: {error}"))?;
        }
        self.debug_put(game, minds);
        Ok(done.len() - 1)
    }

    /// Play one whole turn with the seat's agent, then ask the breakpoints.
    ///
    /// The answer is whether the world moved. Why a run should stop is left
    /// in the timeline for the state document to carry.
    pub fn debug_step(&mut self) -> bool {
        let turn = self.game.turn;
        let before_len = self.game.chronicle.events.len();
        let before_last = self.game.chronicle.events.last().cloned();
        let read_before = self
            .debugger
            .read
            .take()
            .and_then(|read| read.of(&self.game, &self.debugger.breakpoints.conditions))
            .unwrap_or_else(|| readings(&self.game, &self.debugger.breakpoints.conditions));
        let mut read_after = None;
        let era_before = self.game.world_era;
        // In the middle of a round the next whole thing to play is the rest
        // of it: the rivals finish, and the turn that opens is the person's.
        let played = if self.debug_mid_round() {
            self.debug_rivals_play();
            1
        } else {
            self.autoplay(1)
        };
        let fresh = fresh_events(
            before_len,
            before_last.as_ref(),
            turn,
            &self.game.chronicle.events,
        );
        let over = played == 0 || self.game.is_finished() || !self.game.players[0].alive;
        self.debugger.stopped = if over {
            Some(json!({"kind": "end", "turn": self.game.turn}))
        } else {
            self.debugger
                .breakpoints
                .hit(&self.game, fresh)
                .or_else(|| {
                    // The turn that opened is the first of a new world era.
                    (self.debugger.breakpoints.eras && self.game.world_era != era_before).then(
                        || json!({"kind": "era", "turn": self.game.turn, "era": self.game.world_era}),
                    )
                })
                .or_else(|| {
                    let after = readings(&self.game, &self.debugger.breakpoints.conditions);
                    let hit =
                        self.debugger
                            .breakpoints
                            .crossed(self.game.turn, &read_before, &after);
                    read_after = Some(after);
                    hit
                })
        };
        self.debugger.read = read_after
            .filter(|readings| !readings.is_empty())
            .map(|readings| Read {
                log: self.game.log.clone(),
                turn: self.game.turn,
                conditions: self.debugger.breakpoints.conditions.clone(),
                readings,
            });
        played > 0
    }

    /// Play whole turns with the seat's agent, one after another, `turns` of
    /// them at most: what a run of that many steps does, asked for once.
    ///
    /// It stops early where a run stops (a breakpoint, the end of the game),
    /// and why is left in the timeline as a step leaves it. Every turn played
    /// keeps its mark, as a step's does, so any of them is walked back to.
    /// The answer is how the seat stood as each of those turns opened
    /// (`obs::standing`), which is all that something playing a world
    /// forward to see how it comes out reads of the turns on the way.
    pub fn debug_play(&mut self, turns: u32) -> Vec<Value> {
        self.debug_play_read(turns).0
    }

    /// [`Session::debug_play`], with what each turn read for the terms of the
    /// objective a search set (`autosearch::Objective::read`), and the turn a
    /// Must Avoid broken for good ended the play on, if one did. A turn that
    /// breaks one is still played and read: it is where the line ends.
    fn debug_play_read(
        &mut self,
        turns: u32,
    ) -> (Vec<Value>, Vec<super::autosearch::Reading>, Option<Value>) {
        let mut readings = Vec::new();
        let mut terms = Vec::new();
        let mut killed = None;
        for _ in 0..turns {
            if !self.debug_step() {
                break;
            }
            readings.push(crate::obs::standing(&self.game, 0));
            if let Some(objective) = &self.debugger.objective {
                let reading = objective.read(&self.game);
                if let Some(at) = objective.kills(&reading) {
                    killed = Some(json!({
                        "avoid": at,
                        "turn": reading.turn,
                        "key": objective.avoid[at].key,
                    }));
                }
                terms.push(reading);
            }
            if self.debugger.stopped.is_some() || killed.is_some() {
                break;
            }
        }
        (readings, terms, killed)
    }

    /// Play on from the end of what the seated Civ AI has played, `turns` of
    /// them at most, and come back to the open turn (operator 2026-11-24: "we
    /// can compute (a degree in advance) the futures of the paths we are on",
    /// "we will compute the future of each path").
    ///
    /// The turns played are kept ahead of the open one as any played turn is,
    /// so going on from here walks to them instead of playing them again. The
    /// world on the table is the open turn's when this returns, as it stood:
    /// it is refused where the seat has acted in the turn, which coming back
    /// would undo. The answer is how the seat stood as each of those turns
    /// opened, and what happened on the way that is worth seeing coming
    /// (`debug_alerts`).
    pub fn debug_ahead(&mut self, turns: u32) -> Result<(Vec<Value>, Vec<Value>), String> {
        let here = self.game.turn;
        if !self.debugger_turn_stands() {
            return Err(
                "the open turn has been played in; what is ahead is played from its top"
                    .to_string(),
            );
        }
        let Some(home) = self.debugger.mark(here).cloned() else {
            return Err("the open turn is not kept to come back to".to_string());
        };
        let last = self.debug_line().last().map_or(here, |mark| mark.turn);
        if last > here {
            self.debug_jump(last)?;
        }
        let from = self.game.turn;
        let (played, _, _) = self.debug_play_read(turns);
        let alerts = self.debug_alerts(from);
        // Home: the open turn's own mark where it is still kept, and the
        // world it held where the marks ahead have thinned it away.
        let home = self.debugger.mark(here).cloned().unwrap_or(home);
        self.debug_put(home.game, home.minds);
        // Why the play ahead stopped is not why the open turn would.
        self.debugger.stopped = None;
        self.debugger.read = None;
        Ok((played, alerts))
    }

    /// What has happened since turn `from` that a page showing the turns
    /// ahead puts a sign up for: every nuclear detonation in the world, what
    /// the played seat was told that was too consequential to scroll past,
    /// and the end of the game. In the order it happened, the first two
    /// dozen.
    fn debug_alerts(&self, from: u32) -> Vec<Value> {
        const MOST: usize = 24;
        let game = &self.game;
        let mut alerts: Vec<(u32, Value)> = Vec::new();
        for strike in game.chronicle.nuclear_strikes.iter() {
            if strike.turn < from {
                continue;
            }
            let device = if strike.thermonuclear {
                "thermonuclear device"
            } else {
                "nuclear device"
            };
            let who = game
                .players
                .get(strike.attacker)
                .map_or("A civilization", |player| player.civ.as_str());
            alerts.push((
                strike.turn,
                json!({"turn": strike.turn, "kind": "nuclear", "text": format!("{who} detonates a {device}")}),
            ));
        }
        for event in game.events_for(0) {
            if event.turn < from || !event.important || event.category == "Nuclear" {
                continue;
            }
            alerts.push((
                event.turn,
                json!({"turn": event.turn, "kind": event.category.to_lowercase(), "text": event.text}),
            ));
        }
        if game.is_finished() || !game.players[0].alive {
            let text = match game.winner {
                Some(0) => "The seat wins the game".to_string(),
                Some(winner) => format!(
                    "{} wins the game",
                    game.players
                        .get(winner)
                        .map_or("A rival", |player| player.civ.as_str())
                ),
                None if !game.players[0].alive => "The seat is out of the game".to_string(),
                None => "The game ends".to_string(),
            };
            let kind = if game.winner == Some(0) {
                "won"
            } else {
                "over"
            };
            alerts.push((
                game.turn,
                json!({"turn": game.turn, "kind": kind, "text": text}),
            ));
        }
        alerts.sort_by_key(|(turn, _)| *turn);
        alerts
            .into_iter()
            .take(MOST)
            .map(|(_, alert)| alert)
            .collect()
    }

    /// What an agent decides that is not an action: its standing preferences
    /// for the governors of its cities. The agent's own plan-then-replay path
    /// carries the same fields across ("These preferences are decisions",
    /// `ai/advanced.rs`), and a turn replayed without them is not its turn.
    fn debug_carry_preferences(&mut self, played: &Game) {
        self.game.players[0].citizen_food_bias = played.players[0].citizen_food_bias;
        self.game.players[0].city_directives = played.players[0].city_directives.clone();
        self.game.heist_reads_the_host_menu = played.heist_reads_the_host_menu;
        self.game.board_written();
    }

    /// The rivals take their turns until the round comes back to the seat.
    /// `act` does this after the seat ends its turn; this is for a turn the
    /// agent ended by itself.
    fn debug_rivals_play(&mut self) {
        let g = &mut self.game;
        let mut guard = 0;
        while !g.is_finished()
            && g.current != 0
            && g.players[0].alive
            && guard < 2 * g.players.len()
        {
            let pid = g.current;
            self.ais[pid].take_turn(g, pid);
            if g.current == pid && !g.is_finished() {
                let _ = g.apply(pid, &Action::EndTurn);
            }
            guard += 1;
        }
        self.debugger_turn_opened(true);
    }

    /// Whether a round is under way: the played seat has ended its turn and
    /// the round has not come back to it. Only `debug_step_seat` leaves the
    /// world here; everything else plays a round through.
    fn debug_mid_round(&self) -> bool {
        self.game.current != 0 && !self.game.is_finished() && self.game.players[0].alive
    }

    /// Step one seat: the round, a civilization at a time.
    ///
    /// On the played seat's own turn this ends that turn as it stands and
    /// stops there, before any rival has moved. After that each step plays
    /// whoever is up, a rival, a city-state or the barbarians, with that
    /// seat's own agent, and stops again. When the round comes back to the
    /// played seat its next turn opens exactly as it does after an ordinary
    /// end of turn, so a round taken apart is the same round as one taken
    /// whole.
    pub fn debug_step_seat(&mut self) -> Result<(), String> {
        if self.game.is_finished() || !self.game.players[0].alive {
            return Err("this game has no turn left to play".to_string());
        }
        let turn = self.game.turn;
        let seat = self.game.current;
        let before = self.game.log.len();
        if seat == 0 {
            self.game.apply(0, &Action::EndTurn)?;
            self.debugger.plan = None;
        } else {
            let g = &mut self.game;
            self.ais[seat].take_turn(g, seat);
            if g.current == seat && !g.is_finished() {
                let _ = g.apply(seat, &Action::EndTurn);
            }
        }
        let actions = self.game.log.len() - before;
        if self.game.current == 0 && !self.game.is_finished() {
            self.debugger_turn_opened(true);
        }
        self.debugger.stepped = Some(json!({
            "turn": turn,
            "seat": seat,
            "actions": actions,
            "next": self.game.current,
        }));
        Ok(())
    }

    /// Step in: the seat's agent takes the next action of its turn, and only
    /// that. With nothing left to do, the step ends the turn.
    ///
    /// The agent plays the whole turn once, on a copy, and the plan is kept
    /// for the steps that follow. Each step applies the plan's next action
    /// to the real world, so the turn is seen being put together. The last
    /// step puts the agent's own copy on the table and ends the turn there,
    /// which makes a turn taken apart the same turn as one taken whole.
    ///
    /// The plan is made again if anything else has touched the world in
    /// between: an action by hand, an undo, a layer. An action the real
    /// world refuses is passed over.
    pub fn debug_step_action(&mut self) -> Result<(), String> {
        // In the middle of a round the next thing that happens is a rival's
        // turn, and that is the smallest step there is to take.
        if self.debug_mid_round() {
            return self.debug_step_seat();
        }
        if self.game.current != 0 || self.game.is_finished() || !self.game.players[0].alive {
            return Err("this game has no turn left to play".to_string());
        }
        let current = self
            .debugger
            .plan
            .as_ref()
            .is_some_and(|plan| plan.turn == self.game.turn && plan.at == self.game.log.len());
        if !current {
            let whole = self.debugger_turn_stands();
            let mut trial = self.game.clone();
            let before = trial.log.len();
            self.ais[0].take_turn(&mut trial, 0);
            let rest = trial
                .log
                .since(before)
                .filter(|(seat, action)| *seat == 0 && !matches!(action, Action::EndTurn))
                .map(|(_, action)| action.clone())
                .collect();
            self.debug_carry_preferences(&trial);
            self.debugger.plan = Some(Plan {
                turn: self.game.turn,
                at: self.game.log.len(),
                rest,
                whole,
                done: trial,
            });
        }
        let turn = self.game.turn;
        while let Some(action) = self
            .debugger
            .plan
            .as_mut()
            .and_then(|plan| plan.rest.pop_front())
        {
            if self.game.apply(0, &action).is_err() {
                continue;
            }
            let at = self.game.log.len();
            let plan = self.debugger.plan.as_mut().expect("the plan being walked");
            plan.at = at;
            self.debugger.stepped = Some(json!({
                "turn": turn,
                "action": action,
                "left": plan.rest.len(),
            }));
            return Ok(());
        }
        // The plan is spent: what the agent does next is end the turn, on
        // the world it played.
        let plan = self.debugger.plan.take().expect("the plan just walked");
        let whole = plan.whole;
        self.game = plan.done;
        if self.game.current == 0 && !self.game.is_finished() {
            if let Some(error) = self.act(&json!({"type": "end_turn"})) {
                return Err(error);
            }
        } else {
            self.debug_rivals_play();
        }
        // A turn taken apart is the agent's turn all the same.
        self.debugger_agent_played(turn, whole);
        self.debugger.stepped = Some(json!({
            "turn": turn,
            "action": {"type": "end_turn"},
            "left": 0,
        }));
        Ok(())
    }

    /// Let the seat's agent take the decisions in the layers switched on, and
    /// leave the rest of the turn where it is.
    ///
    /// The agent plays the turn on a copy. Its actions are then offered to
    /// the real world in the order it took them, a layer at a time; one that
    /// no longer applies there, because it leaned on something left to the
    /// person, is dropped.
    pub fn debug_assist(&mut self) -> usize {
        if self.debugger.layers.is_empty()
            || self.game.current != 0
            || self.game.is_finished()
            || !self.game.players[0].alive
        {
            return 0;
        }
        let mut trial = self.game.clone();
        let before = trial.log.len();
        self.ais[0].take_turn(&mut trial, 0);
        let planned: Vec<Action> = trial
            .log
            .since(before)
            .filter(|(seat, _)| *seat == 0)
            .map(|(_, action)| action.clone())
            .collect();
        if self.debugger.layers.contains(&Layer::Cities) {
            self.debug_carry_preferences(&trial);
        }
        drop(trial);
        let mut applied = 0;
        for action in &planned {
            let wanted =
                Layer::of(action).is_some_and(|layer| self.debugger.layers.contains(&layer));
            if wanted && self.game.current == 0 && self.game.apply(0, action).is_ok() {
                applied += 1;
            }
        }
        applied
    }

    fn debug_set_layers(&mut self, request: &Value) -> Result<(), String> {
        let mut layers = BTreeSet::new();
        for id in request["layers"].as_array().into_iter().flatten() {
            let id = id.as_str().unwrap_or("");
            layers.insert(Layer::parse(id).ok_or_else(|| format!("no layer named {id:?}"))?);
        }
        self.debugger.layers = layers;
        Ok(())
    }

    /// `POST /debug`: one debugger operation, answered with the world after
    /// it. A refused operation leaves the world as it was and says why.
    pub fn debug(&mut self, request: &Value) -> Value {
        // A step and the layers are played by whichever agent holds the seat;
        // a request may name it, exactly as an auto-play request does.
        let seated = match request["strategy"].as_str() {
            Some(name) if self.debuggable() => {
                let held = self.autoplay_strategy.clone();
                let seated = self.seat_strategy_at(0, name);
                // The rest of a turn being stepped in was planned by the
                // agent that held the seat, and is not this one's turn.
                if self.autoplay_strategy != held {
                    self.debugger.plan = None;
                }
                seated
            }
            _ => Ok(()),
        };
        // How the seat stood on each turn a `play` played, what it read for
        // a search's terms, and where a Must Avoid ended it.
        let mut readings: Option<Vec<Value>> = None;
        let mut terms: Option<Vec<super::autosearch::Reading>> = None;
        let mut killed: Option<Value> = None;
        // What a play ahead of the open turn saw coming (`debug_ahead`).
        let mut alerts: Option<Vec<Value>> = None;
        let result: Result<(), String> = if !self.debuggable() {
            Err("the debugger walks a game with a seat of its own".to_string())
        } else if let Err(error) = seated {
            Err(error)
        } else {
            let op = request["op"].as_str().unwrap_or("status");
            // Only a step straight after a step reads the world the last one
            // left; whatever else is asked may have moved it (`Read`).
            if op != "step" {
                self.debugger.read = None;
            }
            match op {
                "status" => Ok(()),
                "back" => self.debug_back().map(|_| ()),
                "forward" => self.debug_forward().map(|_| ()),
                "restart" => self.debug_jump(self.game.turn).map(|_| ()),
                "jump" => match request["turn"]
                    .as_u64()
                    .and_then(|turn| u32::try_from(turn).ok())
                {
                    Some(turn) => self.debug_jump(turn).map(|_| ()),
                    None => Err("jump needs a turn".to_string()),
                },
                // A rival's turn is not the seat's action to take back.
                "undo" | "save_point" if self.debug_mid_round() => Err(
                    "the rivals are playing; go back to the top of the turn, or let the round finish"
                        .to_string(),
                ),
                "undo" => self.debug_undo().map(|_| ()),
                "save_point" => self
                    .debug_save_point(request["name"].as_str().unwrap_or(""))
                    .map(|_| ()),
                "goto_save" | "drop_save" => {
                    match request["id"].as_u64().and_then(|id| u32::try_from(id).ok()) {
                        Some(id) if request["op"] == "goto_save" => {
                            self.debug_goto_save(id).map(|_| ())
                        }
                        Some(id) => self.debug_drop_save(id),
                        None => Err("a save point is named by its id".to_string()),
                    }
                }
                // Where the memory line is, for a page that knows its device.
                "room" => match request["megabytes"].as_u64() {
                    Some(megabytes) => {
                        let megabytes = megabytes.clamp(128, 3072) as usize;
                        self.debugger.memory_line = Some(megabytes << 20);
                        Ok(())
                    }
                    None => Err("room needs megabytes".to_string()),
                },
                // Never refused: with no kept turn to take, the world stays
                // where it is and the run that asked plays on from it.
                "walk" => {
                    let until = request["turn"]
                        .as_u64()
                        .and_then(|turn| u32::try_from(turn).ok());
                    self.debug_walk(until, request["era"] == json!(true));
                    Ok(())
                }
                "step" => {
                    if self.debug_step() {
                        Ok(())
                    } else {
                        Err("this game has no turn left to play".to_string())
                    }
                }
                // A run of steps asked for once (`debug_play`).
                "play" => match request["turns"]
                    .as_u64()
                    .filter(|turns| (1..=u64::from(PLAY_MOST)).contains(turns))
                {
                    Some(turns) => {
                        let (played, read, ended) = self.debug_play_read(turns as u32);
                        let moved = !played.is_empty();
                        readings = Some(played);
                        if self.debugger.objective.is_some() {
                            terms = Some(read);
                            killed = ended;
                        }
                        if moved {
                            Ok(())
                        } else {
                            Err("this game has no turn left to play".to_string())
                        }
                    }
                    None => Err(format!("play needs turns, 1 to {PLAY_MOST}")),
                },
                // The same run, played past the turns kept ahead, with the
                // world left on the open turn (`debug_ahead`).
                "ahead" => match request["turns"]
                    .as_u64()
                    .filter(|turns| (1..=u64::from(PLAY_MOST)).contains(turns))
                {
                    Some(turns) => self.debug_ahead(turns as u32).map(|(played, seen)| {
                        readings = Some(played);
                        alerts = Some(seen);
                    }),
                    None => Err(format!("ahead needs turns, 1 to {PLAY_MOST}")),
                },
                "step_action" => self.debug_step_action(),
                "step_seat" => self.debug_step_seat(),
                // The rest of a round that is being stepped a seat at a time.
                "finish_round" => {
                    if self.debug_mid_round() {
                        self.debug_rivals_play();
                        Ok(())
                    } else {
                        Err("no round is under way".to_string())
                    }
                }
                "assist" => {
                    self.debugger.assisted = self.debug_assist();
                    Ok(())
                }
                "reveal" => match request["on"].as_bool() {
                    Some(on) => {
                        self.debugger.reveal = on;
                        Ok(())
                    }
                    None => Err("reveal needs on: true or false".to_string()),
                },
                "layers" => self.debug_set_layers(request),
                "breakpoints" => {
                    Breakpoints::from_request(request, &self.game).map(|breakpoints| {
                        self.debugger.breakpoints = breakpoints;
                    })
                }
                // What a search is asked for, read on every turn a `play`
                // plays from here (`autosearch.rs`). Each term is read back
                // off this world before it is kept. None clears it.
                "objective" => match &request["objective"] {
                    Value::Null => {
                        self.debugger.objective = None;
                        Ok(())
                    }
                    asked => super::autosearch::Objective::parse(asked).and_then(|objective| {
                        objective.check(&self.game)?;
                        self.debugger.objective = Some(objective);
                        Ok(())
                    }),
                },
                // What a copy of the engine made to search asks of a half
                // of a split (`debug_remember`).
                "remember" => {
                    self.debug_remember();
                    Ok(())
                }
                // A half of a split that is a new branch from the open turn
                // holds nothing past it (`Timeline::branch`).
                "branch" => {
                    self.debugger.branch(self.game.turn);
                    Ok(())
                }
                // These two belong to a split screen and are answered by its
                // panes (`panes.rs`); reaching here means no half was named.
                "adopt" | "copy" => {
                    Err("that closes or copies a split, and this screen is not split".to_string())
                }
                other => Err(format!("no debugger operation named {other:?}")),
            }
        };
        let mut out = if request["lean"] == json!(true) {
            self.debug_lean()
        } else {
            self.state()
        };
        out["error"] = match result {
            Ok(()) => Value::Null,
            Err(error) => json!(error),
        };
        if let Some(readings) = readings {
            out["readings"] = json!(readings);
        }
        if let Some(alerts) = alerts {
            out["alerts"] = json!(alerts);
        }
        if let Some(terms) = terms {
            out["terms"] = json!(terms);
            out["killed"] = killed.unwrap_or(Value::Null);
        }
        out
    }

    /// The answer to an operation that asked to be answered `lean`: where the
    /// world stands, every seat's score, how the played seat stands and what
    /// the debugger holds, and no state document.
    ///
    /// The document is the map as the seat sees it, every city and unit, and
    /// everything the seat may do next. At turn 80 of a six-seat game it took
    /// 9 ms to build, of a jump's 20 and a step's 130, and whatever plays
    /// many futures of a world to compare how they came out reads none of
    /// it. The scores are
    /// every seat's, met or not: this is a reading of the world, as a
    /// breakpoint condition is, not what the seat is shown. `seat` is the
    /// played seat's row of the standings (`obs::standing`): what a search
    /// may be asked to make the most of.
    fn debug_lean(&self) -> Value {
        let game = &self.game;
        let scores: Vec<Value> = game
            .players
            .iter()
            .enumerate()
            .filter(|(_, player)| !player.is_minor && !player.is_barbarian)
            .map(|(seat, player)| {
                json!({"id": seat, "alive": player.alive, "score": game.score(seat)})
            })
            .collect();
        let mut out = json!({
            "lean": true,
            "turn": game.turn,
            "current": game.current,
            "seed": game.seed,
            "finished": game.is_finished(),
            "draw": game.is_draw(),
            "winner": game.winner,
            "scores": scores,
        });
        if self.debuggable() {
            out["seat"] = crate::obs::standing(game, 0);
            out["debug"] = self.debugger_json();
        }
        out
    }
}

#[cfg(test)]
#[path = "debugger_tests.rs"]
mod tests;
