//! The simulator debugger: a played game as a timeline that can be walked.
//!
//! A game somebody is playing used to run one way. This module keeps a mark of
//! the world at the top of every turn the played seat has opened, and that one
//! record is what the debugger's controls are made of:
//!
//! * **back / forward / jump** put a marked turn on the table again. Going
//!   back does not forget the turns ahead: they stay marked until the world
//!   is played forward differently, and then they are replaced.
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

use std::collections::{BTreeSet, VecDeque};

use serde_json::{json, Value};

use super::Session;
use crate::game::{Action, Event, Game};

/// The newest marks are kept one a turn, whatever else has to go.
const DENSE_MARKS: usize = 60;
/// Breakpoint phrases a page may hold at once.
const MAX_PHRASES: usize = 16;
/// Breakpoint turns a page may hold at once.
const MAX_BREAK_TURNS: usize = 64;

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
}

impl Breakpoints {
    fn from_request(request: &Value) -> Result<Breakpoints, String> {
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
        })
    }

    fn json(&self) -> Value {
        json!({
            "turns": self.turns,
            "categories": self.categories,
            "phrases": self.phrases,
            "important": self.important,
            "every_seat": self.every_seat,
            // The vocabulary a page offers, read from the engine's own list.
            "known_categories": crate::game::EVENT_CATEGORIES,
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
/// However little room there is, a turn divisible by this keeps its mark.
const COARSEST_STRIDE: u32 = 8;
/// Memory in use past which the marks start giving room back, unless the
/// page names another line (`room`).
const MEMORY_LINE: usize = 1 << 30;
/// Past the line, each further growth by this much takes room away again.
const MEMORY_STEP: usize = 32 << 20;
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

/// The world as it stood when the played seat's turn opened.
#[derive(Clone)]
struct Mark {
    turn: u32,
    /// How long the action log was. Everything past it in the live game is
    /// what the seat has done with the turn since.
    actions: usize,
    game: Game,
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
        self.marks.push(Mark {
            turn: game.turn,
            actions: game.log.len(),
            game: game.clone(),
        });
        self.thin();
    }

    /// Only the top of the played seat's own turn is a place to come back to.
    fn markable(game: &Game) -> bool {
        game.current == 0 && game.is_human_seat(0)
    }

    /// How many marks may be kept.
    fn room(&self) -> usize {
        self.room.unwrap_or(ROOMY_MARKS).max(DENSE_MARKS)
    }

    /// Hear how much memory is in use. Past the line, the room becomes a
    /// tenth less than what is held, and again each time the memory has grown
    /// by another step: memory that only grows cannot say that it was given
    /// back, so the growing is what is listened to.
    fn squeeze(&mut self, in_use: usize) {
        let line = self.memory_line.unwrap_or(MEMORY_LINE);
        if in_use <= line || in_use < self.squeezed_at.saturating_add(MEMORY_STEP) {
            return;
        }
        self.squeezed_at = in_use;
        let held = self.marks.len().min(self.room());
        self.room = Some((held - held / 10).max(DENSE_MARKS));
    }

    /// Make room when there are more marks than there is room for.
    ///
    /// The first mark and the newest `DENSE_MARKS` always stay. Between them
    /// the oldest turns go first and the odd ones before the even, so a long
    /// game's early turns are kept one in two, then one in four, and never
    /// fewer than one in `COARSEST_STRIDE`.
    fn thin(&mut self) {
        if let Some(in_use) = memory_in_use() {
            self.squeeze(in_use);
        }
        let mut over = self.marks.len().saturating_sub(self.room());
        let mut stride = 2;
        while over > 0 && stride <= COARSEST_STRIDE {
            let old = self.marks.len().saturating_sub(DENSE_MARKS);
            let mut index = 0;
            self.marks.retain(|mark| {
                let goes = over > 0 && index > 0 && index < old && mark.turn % stride != 0;
                index += 1;
                if goes {
                    over -= 1;
                }
                !goes
            });
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

    fn json(&self, game: &Game) -> Value {
        let turns: Vec<u32> = self.marks.iter().map(|mark| mark.turn).collect();
        let acted = self.acted(game);
        json!({
            "turn": game.turn,
            "turns": turns,
            "first": turns.first(),
            "last": turns.last(),
            "acted": acted,
            "can_undo": acted > 0 && self.mark(game.turn).is_some(),
            "can_back": acted > 0 || turns.first().is_some_and(|first| *first < game.turn),
            "can_forward": turns.last().is_some_and(|last| *last > game.turn),
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
        if assist && !self.debugger.layers.is_empty() {
            self.debugger.assisted = self.debug_assist();
        }
    }

    /// Whether the page is shown the world with the fog lifted.
    pub(super) fn debugger_reveals(&self) -> bool {
        self.debugger.reveal
    }

    /// The debugger's part of the state document.
    pub(super) fn debugger_json(&self) -> Value {
        self.debugger.json(&self.game)
    }

    /// Put a marked world on the table in place of the live one.
    ///
    /// The agents are rebuilt, as they are for a loaded save: their plans
    /// belong to the future this world is being taken out of.
    fn debug_put(&mut self, game: Game) {
        let seat = self.autoplay_strategy.clone();
        let rivals = self.rival_strategy.clone();
        let view = self.view_player;
        let humans = std::mem::take(&mut self.human_players);
        let debugger = std::mem::take(&mut self.debugger);
        let resumed = self.resumed_next_game_params.take();
        let (rival_yields, own_yields) = (self.keeps_rival_yields, self.keeps_own_yields);
        let mut next = Session::from_saved_game(self.params.clone(), game);
        if let Some(name) = rivals {
            let _ = next.seat_rivals(&name);
        }
        if let Some(name) = seat {
            let _ = next.seat_strategy_at(0, &name);
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
        next.debugger.stopped = None;
        next.keeps_rival_yields = self.keeps_rival_yields;
        next.keeps_own_yields = self.keeps_own_yields;
        next
    }

    /// Go to a marked turn. A turn with no mark of its own answers with the
    /// nearest one before it.
    pub fn debug_jump(&mut self, turn: u32) -> Result<u32, String> {
        let Some(mark) = self.debugger.mark_at_or_before(turn) else {
            return Err(format!("turn {turn} is before this timeline begins"));
        };
        let (reached, game) = (mark.turn, mark.game.clone());
        self.debug_put(game);
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

    /// Keep the world as it stands under a name, to come back to.
    pub fn debug_save_point(&mut self, name: &str) -> Result<u32, String> {
        let name: String = name.trim().chars().take(MAX_SAVE_NAME).collect();
        if name.is_empty() {
            let turn = self.game.turn;
            return self
                .debugger
                .keep(&format!("Turn {turn}"), &self.game, false);
        }
        self.debugger.keep(&name, &self.game, false)
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
        let (game, top) = (save.game.clone(), save.top.clone());
        let left = &self.game;
        let kept = self
            .debugger
            .saves
            .iter()
            .any(|save| save.game.log.len() == left.log.len() && on_one_line(&save.game, left));
        if !kept {
            // With no room for it the world left is simply not kept.
            let _ = self.debugger.keep(WHERE_YOU_WERE, &self.game, true);
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
        self.debug_put(game);
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
        let mut game = mark.game.clone();
        for (seat, action) in &done[..done.len() - 1] {
            game.apply(*seat, action)
                .map_err(|error| format!("the turn could not be replayed: {error}"))?;
        }
        self.debug_put(game);
        Ok(done.len() - 1)
    }

    /// Play one whole turn with the seat's agent, then ask the breakpoints.
    ///
    /// The answer is whether the world moved. Why a run should stop is left
    /// in the timeline for the state document to carry.
    pub fn debug_step(&mut self) -> bool {
        let turn = self.game.turn;
        let before_len = self.game.events.len();
        let before_last = self.game.events.last().cloned();
        let played = self.autoplay(1);
        let fresh = fresh_events(before_len, before_last.as_ref(), turn, &self.game.events);
        let over = played == 0 || self.game.is_finished() || !self.game.players[0].alive;
        self.debugger.stopped = if over {
            Some(json!({"kind": "end", "turn": self.game.turn}))
        } else {
            self.debugger.breakpoints.hit(&self.game, fresh)
        };
        played > 0
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
        if self.game.current != 0 || self.game.is_finished() || !self.game.players[0].alive {
            return Err("this game has no turn left to play".to_string());
        }
        let current = self
            .debugger
            .plan
            .as_ref()
            .is_some_and(|plan| plan.turn == self.game.turn && plan.at == self.game.log.len());
        if !current {
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
        self.game = plan.done;
        if self.game.current == 0 && !self.game.is_finished() {
            if let Some(error) = self.act(&json!({"type": "end_turn"})) {
                return Err(error);
            }
        } else {
            self.debug_rivals_play();
        }
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
            Some(name) if self.debuggable() => self.seat_strategy_at(0, name),
            _ => Ok(()),
        };
        let result: Result<(), String> = if !self.debuggable() {
            Err("the debugger walks a game with a seat of its own".to_string())
        } else if let Err(error) = seated {
            Err(error)
        } else {
            match request["op"].as_str().unwrap_or("status") {
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
                "step" => {
                    if self.debug_step() {
                        Ok(())
                    } else {
                        Err("this game has no turn left to play".to_string())
                    }
                }
                "step_action" => self.debug_step_action(),
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
                "breakpoints" => Breakpoints::from_request(request).map(|breakpoints| {
                    self.debugger.breakpoints = breakpoints;
                }),
                other => Err(format!("no debugger operation named {other:?}")),
            }
        };
        let mut out = self.state();
        out["error"] = match result {
            Ok(()) => Value::Null,
            Err(error) => json!(error),
        };
        out
    }
}

#[cfg(test)]
#[path = "debugger_tests.rs"]
mod tests;
