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
//! * **layers** are the rungs between playing every decision by hand and
//!   handing the seat over: research, cities, the empire's standing choices,
//!   diplomacy, units. The agent plans the turn on a copy of the world, and
//!   only the actions in a layer that is switched on are applied to the real
//!   one. The rest of the turn is left to the person.
//!
//! A mark is a clone of the game. The game shares what a turn did not change
//! with its clones, so a mark costs the turn's difference rather than the
//! world. Old marks are still thinned (`DENSE_MARKS`, `SPARSE_EVERY`) because
//! a browser tab's memory only grows.
//!
//! A save carries the world and not this record: a loaded game starts a
//! timeline of its own from the turn it resumes.

use std::collections::BTreeSet;

use serde_json::{json, Value};

use super::Session;
use crate::game::{Action, Event, Game};

/// The newest marks are kept one a turn.
const DENSE_MARKS: usize = 60;
/// Behind those, one turn in this many keeps its mark.
const SPARSE_EVERY: u32 = 10;
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

/// The world as it stood when the played seat's turn opened.
struct Mark {
    turn: u32,
    /// How long the action log was. Everything past it in the live game is
    /// what the seat has done with the turn since.
    actions: usize,
    game: Game,
}

/// A played game's marks, and what the debugger was told to do with it.
#[derive(Default)]
pub struct Timeline {
    /// Oldest first, one a turn at most.
    marks: Vec<Mark>,
    breakpoints: Breakpoints,
    layers: BTreeSet<Layer>,
    /// Why the last step stopped a run, until the next thing moves the world.
    stopped: Option<Value>,
    /// What the layers did when the open turn began.
    assisted: usize,
}

impl Timeline {
    /// Start over on this world: a new game, or a loaded one.
    fn begin(&mut self, game: &Game) {
        self.marks.clear();
        self.stopped = None;
        self.assisted = 0;
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

    fn thin(&mut self) {
        if self.marks.len() <= DENSE_MARKS {
            return;
        }
        let dense_from = self.marks.len() - DENSE_MARKS;
        let mut index = 0;
        self.marks.retain(|mark| {
            let keep = index >= dense_from || index == 0 || mark.turn % SPARSE_EVERY == 0;
            index += 1;
            keep
        });
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
        })
    }
}

/// The events a step added to a log that is trimmed from its front.
fn fresh_events(
    before_len: usize,
    before_last: Option<&Event>,
    turn: u32,
    now: &[Event],
) -> &[Event] {
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
        self.debugger.assisted = 0;
        self.debugger.record(&self.game);
        if assist && !self.debugger.layers.is_empty() {
            self.debugger.assisted = self.debug_assist();
        }
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
        self.debugger.assisted = 0;
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
        self.debugger.stopped = if played == 0 {
            Some(json!({"kind": "end", "turn": self.game.turn}))
        } else if self.game.is_finished() || !self.game.players[0].alive {
            Some(json!({"kind": "end", "turn": self.game.turn}))
        } else {
            self.debugger.breakpoints.hit(&self.game, fresh)
        };
        played > 0
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
                "step" => {
                    if self.debug_step() {
                        Ok(())
                    } else {
                        Err("this game has no turn left to play".to_string())
                    }
                }
                "assist" => {
                    self.debugger.assisted = self.debug_assist();
                    Ok(())
                }
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
