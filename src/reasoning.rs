//! What the agents were thinking, for observers only.
//!
//! The spectator can already see what a civilization *did* — the event log
//! reports the outcome and the war log keeps the account. Neither says why.
//! An agent that razes a city, drops a policy card, or refuses an alliance is
//! working from numbers it compared a moment earlier, and until now those
//! numbers existed only inside one stack frame.
//!
//! A [`Journal`] is the sink the agents write those comparisons into. Nothing
//! here is read back by the simulation: a journal that records nothing plays
//! exactly the same game as one that records everything, which is what makes
//! it safe to leave the recording off everywhere except the seats a person is
//! actually watching.
//!
//! Three properties do the work:
//!
//! - **Off is free.** A journal with no sink is one `Option` test, and the
//!   [`think!`] macro puts the `format!` calls *inside* that test, so a
//!   headless tournament never builds a string it will not keep.
//! - **A clone records nothing.** Cloning an agent is how a rollout is made
//!   (`StrategicAi` clones its inner agent to play out a hypothetical), and a
//!   hypothetical agent's thoughts are not what the observer is watching.
//!   `Journal`'s `Clone` therefore returns a silent journal rather than a
//!   second handle on the same sink. Sharing a sink on purpose — the agent
//!   with its own `BasicAi` layer, or a whole table of seats — goes through
//!   [`Journal::handle`], which says so at the call site.
//! - **The ring is bounded twice.** A cap on the whole log keeps a long game's
//!   memory flat, and a separate per-turn budget stops one pathological turn
//!   from evicting every other civilization's reasoning out of the window the
//!   observer is reading.

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use crate::Pos;

/// How deep in the decision hierarchy a thought sits. The observer filters on
/// this: a viewer who wants to follow the shape of a game reads `Strategy`
/// alone, and one auditing a specific move reads everything.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Level {
    /// Empire-level intent: the grand strategy, the victory lane, who the
    /// campaign is against, what is threatening home. A handful per turn.
    Strategy = 0,
    /// A committed choice — a tech started, a card slotted, a city's
    /// production set, a war declared, a settle site claimed.
    Decision = 1,
    /// The working underneath a choice: candidates that were scored and
    /// rejected, individual unit steps, per-tile comparisons.
    Detail = 2,
}

impl Level {
    pub fn as_str(self) -> &'static str {
        match self {
            Level::Strategy => "strategy",
            Level::Decision => "decision",
            Level::Detail => "detail",
        }
    }
}

/// Which part of the agent was thinking. These are the passes
/// `AdvancedAi::take_turn` runs, grouped the way an observer would ask about
/// them rather than the way the code is laid out.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Topic {
    /// Grand strategy, victory lane, campaign target, threat assessment.
    Strategy,
    /// Technologies and civics.
    Research,
    /// Government form and governors.
    Government,
    /// Policy cards.
    Policies,
    /// City production: units, buildings, districts, projects, wonders.
    Cities,
    /// Settling, settler and builder movement, tile improvements.
    Expansion,
    /// War and peace, force groups, attacks, unit posture, promotions.
    Military,
    /// Deals, alliances, envoys, congress votes, espionage.
    Diplomacy,
    /// Gold: purchases, trade routes, patronage.
    Economy,
    /// Religion: pantheons, beliefs, missionaries, faith spending.
    Faith,
}

impl Topic {
    pub fn as_str(self) -> &'static str {
        match self {
            Topic::Strategy => "strategy",
            Topic::Research => "research",
            Topic::Government => "government",
            Topic::Policies => "policies",
            Topic::Cities => "cities",
            Topic::Expansion => "expansion",
            Topic::Military => "military",
            Topic::Diplomacy => "diplomacy",
            Topic::Economy => "economy",
            Topic::Faith => "faith",
        }
    }

    /// The label the observer's topic filter shows. Kept beside `as_str` so
    /// the wire name and the human name cannot drift apart.
    pub fn label(self) -> &'static str {
        match self {
            Topic::Strategy => "Grand strategy",
            Topic::Research => "Research & civics",
            Topic::Government => "Government",
            Topic::Policies => "Policy cards",
            Topic::Cities => "City production",
            Topic::Expansion => "Expansion & land",
            Topic::Military => "War & units",
            Topic::Diplomacy => "Diplomacy",
            Topic::Economy => "Gold & trade",
            Topic::Faith => "Religion",
        }
    }

    /// Every topic, in the order the filter lists them.
    pub const ALL: [Topic; 10] = [
        Topic::Strategy,
        Topic::Research,
        Topic::Government,
        Topic::Policies,
        Topic::Cities,
        Topic::Expansion,
        Topic::Military,
        Topic::Diplomacy,
        Topic::Economy,
        Topic::Faith,
    ];
}

/// A rule key as prose.
///
/// Everything the engine names itself by is a snake_case key — `code_of_laws`,
/// `divine_spark`, `battering_ram` — and a sentence built out of those reads
/// like a config file. The log is the one place in CIVVIS a person reads these
/// keys in running text, so they lose their underscores on the way in. The
/// case is left alone: these appear mid-sentence, and a Title Cased noun in
/// the middle of a lower-case clause is worse than the underscore was.
pub fn plain(key: &str) -> String {
    key.replace('_', " ")
}

/// One thing an agent decided, and the reason it gave.
#[derive(Clone, Debug, PartialEq)]
pub struct Thought {
    /// Monotonic within a game. The observer polls with the last id it holds
    /// and is sent only what came after it.
    pub id: u64,
    pub turn: u32,
    pub player: usize,
    pub topic: Topic,
    pub level: Level,
    /// What was decided, in one line.
    pub headline: String,
    /// Why — the numbers that were compared, the runner-up, the constraint
    /// that bound. Empty when the headline is the whole of it.
    pub detail: String,
    /// Where on the map this happened, when it happened somewhere.
    pub focus: Option<Pos>,
}

/// How many thoughts a game keeps. At the observed rate of a few hundred a
/// turn this is roughly the last twenty turns, which is what "watch the AI
/// think, turn by turn" needs; older reasoning is reported as dropped rather
/// than silently forgotten.
const RING_CAPACITY: usize = 6000;

/// How many thoughts one civilization may record in one of its turns. A turn
/// that somehow produced thousands would otherwise push every other
/// civilization's reasoning out of the window before anyone could read it.
const TURN_BUDGET: usize = 600;

struct Log {
    turn: u32,
    player: usize,
    next_id: u64,
    /// Thoughts evicted by the ring, so the observer can say its history is
    /// truncated instead of quietly showing a shorter game.
    dropped: u64,
    /// Civilization-turns cut short by the per-turn budget. Counted
    /// separately from `dropped`: this is one agent being unusually noisy, not
    /// the observer falling behind the whole game.
    truncated_turns: u64,
    spent_this_turn: usize,
    /// Whether this turn has already been counted against `truncated_turns`.
    truncated: bool,
    ceiling: Level,
    thoughts: VecDeque<Thought>,
}

impl Log {
    /// The budget gate. Refusing is what stops one civilization's turn from
    /// evicting every other civilization's reasoning, and the first refusal in
    /// a turn records that the turn is not the whole story.
    fn within_budget(&mut self) -> bool {
        if self.spent_this_turn < TURN_BUDGET {
            return true;
        }
        if !self.truncated {
            self.truncated = true;
            self.truncated_turns += 1;
        }
        false
    }
}

impl Default for Log {
    fn default() -> Log {
        Log {
            turn: 0,
            player: 0,
            next_id: 1,
            dropped: 0,
            truncated_turns: 0,
            spent_this_turn: 0,
            truncated: false,
            ceiling: Level::Detail,
            thoughts: VecDeque::new(),
        }
    }
}

/// A handle on one game's reasoning log. Default is off.
///
/// See the module documentation for why `Clone` deliberately does not clone
/// the sink.
#[derive(Default)]
pub struct Journal(Option<Arc<Mutex<Log>>>);

impl Clone for Journal {
    /// A cloned agent is a hypothetical agent. Rollouts clone an agent and
    /// play it forward over a cloned game; those thoughts belong to a future
    /// that will not happen, and mixing them into the observer's log would
    /// report a plan the world never carried out. So a clone is silent, and
    /// deliberate sharing is spelled [`Journal::handle`].
    fn clone(&self) -> Journal {
        Journal(None)
    }
}

impl Journal {
    /// A live journal, recording everything up to `Level::Detail`.
    pub fn recording() -> Journal {
        Journal(Some(Arc::new(Mutex::new(Log::default()))))
    }

    /// A second handle on the *same* log. This is how one game's seats — and
    /// the layers inside one agent — share a single ordered record.
    pub fn handle(&self) -> Journal {
        Journal(self.0.clone())
    }

    /// Whether anything is listening.
    pub fn is_recording(&self) -> bool {
        self.0.is_some()
    }

    /// Whether a thought at this level would be kept. The [`think!`] macro
    /// tests this before formatting anything, so this is also where the
    /// per-turn budget is spent: a refusal here is what marks the acting
    /// civilization's turn as truncated.
    pub fn wants(&self, level: Level) -> bool {
        let Some(log) = &self.0 else { return false };
        let Ok(mut log) = log.lock() else { return false };
        level <= log.ceiling && log.within_budget()
    }

    /// Stop recording below this depth. The observer normally filters client
    /// side — it wants the whole record and its own controls — but an
    /// operator running a very long unattended exhibition can raise the floor
    /// here and keep the ring covering more turns.
    pub fn set_ceiling(&self, ceiling: Level) {
        if let Some(log) = &self.0 {
            if let Ok(mut log) = log.lock() {
                log.ceiling = ceiling;
            }
        }
    }

    /// Stamp the context every following thought is recorded under, and start
    /// this civilization's turn budget again. Called once at the top of an
    /// agent's turn; the agents themselves never repeat the turn number or
    /// their own id at a call site.
    pub fn begin_turn(&self, turn: u32, player: usize) {
        if let Some(log) = &self.0 {
            if let Ok(mut log) = log.lock() {
                log.turn = turn;
                log.player = player;
                log.spent_this_turn = 0;
                log.truncated = false;
            }
        }
    }

    /// Record a thought. Prefer the [`think!`] macro, which keeps the
    /// formatting out of a game that is not being watched.
    pub fn note(
        &self,
        topic: Topic,
        level: Level,
        headline: String,
        detail: String,
        focus: Option<Pos>,
    ) {
        let Some(log) = &self.0 else { return };
        let Ok(mut log) = log.lock() else { return };
        if level > log.ceiling || !log.within_budget() {
            return;
        }
        log.spent_this_turn += 1;
        let thought = Thought {
            id: log.next_id,
            turn: log.turn,
            player: log.player,
            topic,
            level,
            headline,
            detail,
            focus,
        };
        log.next_id += 1;
        log.thoughts.push_back(thought);
        while log.thoughts.len() > RING_CAPACITY {
            log.thoughts.pop_front();
            log.dropped += 1;
        }
    }

    /// Everything recorded after `cursor`, the cursor to ask with next time,
    /// and how much has been lost off each end of the ring.
    ///
    /// A cursor from a previous game — the exhibition starts a new world with
    /// a fresh log — is ahead of `next_id`, and is answered with the whole
    /// window rather than with nothing.
    pub fn since(&self, cursor: u64) -> Delta {
        let Some(log) = &self.0 else {
            return Delta::default();
        };
        let Ok(log) = log.lock() else {
            return Delta::default();
        };
        let stale = cursor >= log.next_id;
        let thoughts: Vec<Thought> = log
            .thoughts
            .iter()
            .filter(|thought| stale || thought.id > cursor)
            .cloned()
            .collect();
        Delta {
            cursor: log.next_id.saturating_sub(1),
            reset: stale,
            dropped: log.dropped,
            truncated_turns: log.truncated_turns,
            thoughts,
        }
    }

    /// How many thoughts the log is currently holding.
    pub fn len(&self) -> usize {
        match &self.0 {
            None => 0,
            Some(log) => log.lock().map(|log| log.thoughts.len()).unwrap_or(0),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// One answer to "what has been thought since I last asked".
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Delta {
    /// The id to ask with next time.
    pub cursor: u64,
    /// The reader's cursor named a log this is not — a new world. Everything
    /// it holds should be discarded before these thoughts are added.
    pub reset: bool,
    pub dropped: u64,
    pub truncated_turns: u64,
    pub thoughts: Vec<Thought>,
}

/// Record one thought, formatting nothing when nobody is watching.
///
/// ```ignore
/// think!(journal, Research, Decision, "Researching {tech}");
/// think!(journal, Policies, Decision, "Slotted {card}";
///        "worth {value:.0} against {runner_up} at {second:.0}");
/// think!(journal, Military, Detail, "{unit} attacks {city}";
///        "{odds:.0}% to win"; pos);
/// ```
///
/// The three sections are headline, reason, and where on the map it happened,
/// separated by `;`. Only the headline is required.
#[macro_export]
macro_rules! think {
    ($journal:expr, $topic:ident, $level:ident, $head:literal $(, $ha:expr)* $(,)?) => {
        if $journal.wants($crate::reasoning::Level::$level) {
            $journal.note(
                $crate::reasoning::Topic::$topic,
                $crate::reasoning::Level::$level,
                format!($head $(, $ha)*),
                String::new(),
                None,
            );
        }
    };
    ($journal:expr, $topic:ident, $level:ident,
     $head:literal $(, $ha:expr)* ; $why:literal $(, $wa:expr)* $(,)?) => {
        if $journal.wants($crate::reasoning::Level::$level) {
            $journal.note(
                $crate::reasoning::Topic::$topic,
                $crate::reasoning::Level::$level,
                format!($head $(, $ha)*),
                format!($why $(, $wa)*),
                None,
            );
        }
    };
    ($journal:expr, $topic:ident, $level:ident,
     $head:literal $(, $ha:expr)* ; $why:literal $(, $wa:expr)* ; $focus:expr $(,)?) => {
        if $journal.wants($crate::reasoning::Level::$level) {
            $journal.note(
                $crate::reasoning::Topic::$topic,
                $crate::reasoning::Level::$level,
                format!($head $(, $ha)*),
                format!($why $(, $wa)*),
                Some($focus),
            );
        }
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_journal_that_is_off_keeps_nothing() {
        let journal = Journal::default();
        assert!(!journal.is_recording());
        assert!(!journal.wants(Level::Strategy));
        think!(journal, Strategy, Strategy, "never recorded");
        assert!(journal.is_empty());
        assert_eq!(journal.since(0), Delta::default());
    }

    #[test]
    fn a_cloned_agents_journal_is_silent_so_rollouts_do_not_pollute_the_log() {
        let journal = Journal::recording();
        journal.begin_turn(4, 1);
        think!(journal, Strategy, Strategy, "the world");
        let rollout = journal.clone();
        rollout.begin_turn(4, 1);
        think!(rollout, Strategy, Strategy, "a future that will not happen");
        assert!(!rollout.is_recording());
        assert_eq!(journal.len(), 1);
        assert_eq!(journal.since(0).thoughts[0].headline, "the world");
    }

    #[test]
    fn a_handle_shares_the_one_ordered_record() {
        let journal = Journal::recording();
        let layer = journal.handle();
        journal.begin_turn(7, 2);
        think!(journal, Strategy, Strategy, "outer");
        think!(layer, Cities, Decision, "inner");
        let delta = journal.since(0);
        assert_eq!(delta.thoughts.len(), 2);
        // The shared log is one clock: the layer's thought is stamped with the
        // turn and player the outer agent opened, not with a context of its
        // own.
        assert!(delta.thoughts.iter().all(|t| t.turn == 7 && t.player == 2));
        assert_eq!(delta.thoughts[1].topic, Topic::Cities);
    }

    #[test]
    fn a_cursor_is_answered_with_only_what_came_after_it() {
        let journal = Journal::recording();
        journal.begin_turn(1, 0);
        think!(journal, Research, Decision, "first");
        let first = journal.since(0);
        assert_eq!(first.thoughts.len(), 1);
        assert!(!first.reset);
        think!(journal, Research, Decision, "second");
        let second = journal.since(first.cursor);
        assert_eq!(second.thoughts.len(), 1);
        assert_eq!(second.thoughts[0].headline, "second");
        // Asking again with nothing new is an empty answer, not a repeat.
        assert!(journal.since(second.cursor).thoughts.is_empty());
    }

    #[test]
    fn a_cursor_from_a_previous_world_is_answered_with_the_whole_window() {
        let journal = Journal::recording();
        journal.begin_turn(1, 0);
        think!(journal, Strategy, Strategy, "a new world's first thought");
        let delta = journal.since(9_000);
        assert!(delta.reset);
        assert_eq!(delta.thoughts.len(), 1);
    }

    #[test]
    fn the_ring_is_bounded_and_says_what_it_dropped() {
        let journal = Journal::recording();
        for turn in 0..(RING_CAPACITY / TURN_BUDGET + 2) {
            journal.begin_turn(turn as u32, 0);
            for _ in 0..TURN_BUDGET {
                think!(journal, Cities, Detail, "a thought");
            }
        }
        assert_eq!(journal.len(), RING_CAPACITY);
        assert!(journal.since(0).dropped > 0);
    }

    #[test]
    fn one_turn_cannot_spend_the_whole_ring() {
        let journal = Journal::recording();
        journal.begin_turn(1, 0);
        for _ in 0..(TURN_BUDGET * 3) {
            think!(journal, Military, Detail, "a very busy turn");
        }
        assert_eq!(journal.len(), TURN_BUDGET);
        assert_eq!(journal.since(0).truncated_turns, 1);
        // The next civilization to act is not starved by it.
        journal.begin_turn(1, 1);
        think!(journal, Strategy, Strategy, "the next seat still thinks");
        assert_eq!(journal.len(), TURN_BUDGET + 1);
    }

    #[test]
    fn the_ceiling_refuses_detail_without_touching_the_levels_above_it() {
        let journal = Journal::recording();
        journal.set_ceiling(Level::Decision);
        journal.begin_turn(1, 0);
        think!(journal, Military, Detail, "a unit step");
        think!(journal, Military, Decision, "a war declaration");
        let delta = journal.since(0);
        assert_eq!(delta.thoughts.len(), 1);
        assert_eq!(delta.thoughts[0].level, Level::Decision);
    }

    #[test]
    fn a_thought_carries_its_reason_and_where_it_happened() {
        let journal = Journal::recording();
        journal.begin_turn(12, 3);
        let site = (4, -2);
        think!(journal, Expansion, Decision, "Settling {:?}", site;
               "worth {:.1}, best of {} sites", 18.5, 6; site);
        let thought = &journal.since(0).thoughts[0];
        assert_eq!(thought.headline, "Settling (4, -2)");
        assert_eq!(thought.detail, "worth 18.5, best of 6 sites");
        assert_eq!(thought.focus, Some((4, -2)));
        assert_eq!(thought.turn, 12);
        assert_eq!(thought.player, 3);
    }

    #[test]
    fn every_topic_and_level_has_a_distinct_wire_name() {
        let mut names: Vec<&str> = Topic::ALL.iter().map(|t| t.as_str()).collect();
        names.sort_unstable();
        let count = names.len();
        names.dedup();
        assert_eq!(names.len(), count);
        assert!(Topic::ALL.iter().all(|t| !t.label().is_empty()));
        assert!(Level::Strategy < Level::Decision && Level::Decision < Level::Detail);
    }
}
