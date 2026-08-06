//! The simultaneous turn structure: plan against a shared snapshot, commit
//! under the ordinary rules.
//!
//! Sequential play lets seat N+1 see everything seat N just did, which is
//! also what makes the seats impossible to deliberate in parallel: each
//! seat's information set includes the previous seat's whole turn. The
//! simultaneous structure changes exactly that one thing. At the top of a
//! game turn every seat receives the same view of the world — the world as
//! it stands with nobody having acted this turn — plans its complete turn
//! against a private copy, and the plans are then committed seat by seat
//! through the very same `Game::apply` calls a sequential game makes.
//!
//! Two properties fall out of committing through the ordinary machinery and
//! are load-bearing:
//!
//! - **The committed game is an ordinary game.** Its action log replays
//!   through a plain `apply` loop exactly like a sequential log
//!   ([`replay tests`](self::tests)), saves round-trip, and every
//!   determinism gate the engine already has applies unchanged. The variant
//!   lives entirely in this driver; `game.rs` has no simultaneous code path.
//! - **A plan the world has outrun is dropped, not reinterpreted.** Between
//!   planning and committing, another seat's committed actions may occupy a
//!   tile, kill a target, or take a city. The stale order simply fails
//!   `Game::apply` — which consumes no RNG on failure, the invariant the
//!   replay test enforces — and the drop is counted in the
//!   [`SimultaneousCensus`]. The census is the mode's health instrument:
//!   a rising drop rate is the first sign the regime is distorting play.
//!
//! Planning worlds advance through the seats with an *empty forward*: one
//! rolling clone takes each seat's `EndTurn` with no actions, so a later
//! seat plans with its own upkeep (unit refresh, growth, income) applied but
//! every rival frozen. Each seat's planning copy draws from a seat- and
//! turn-keyed RNG stream — the same discipline disasters and meteors use —
//! so no seat's speculation shares draws with another's or shifts the
//! authoritative stream.
//!
//! What this deliberately does not change: minors and barbarians plan like
//! everyone else; diplomacy already works by deferred `pending_deals`; and
//! the commit order is the stock ascending seat order, so within one turn
//! an earlier seat's orders land first (seat identity is itself seed-random
//! under `randomize_civs`, which is what washes the priority out across a
//! corpus).

use std::collections::BTreeMap;

use crate::action_space::kind_name;
use crate::ai::{run_game, Ai};
use crate::game::{Action, Game};
use crate::rng::Rng;
use crate::setup::TurnStructure;

/// What became of every action the seats planned, over a whole game.
///
/// `planned == applied + dropped` by construction; the other counters are
/// rare-path instruments that should read zero in a healthy run.
#[derive(Clone, Debug, Default, serde::Serialize)]
pub struct SimultaneousCensus {
    /// Actions captured from planning worlds and offered to the commit.
    pub planned: u64,
    /// Planned actions the live world accepted.
    pub applied: u64,
    /// Planned actions the live world refused — the world had outrun them.
    pub dropped: u64,
    /// Refused actions by [`kind_name`], for reading *what* gets outrun.
    pub dropped_by_kind: BTreeMap<&'static str, u64>,
    /// Mandatory choices (a captured city's fate) the plan never made and
    /// the commit resolved with the first legal answer.
    pub forced: u64,
    /// Seats that could not be planned because the rolling forward world
    /// had already decided the game or eliminated them.
    pub unplanned_seats: u64,
    /// Whole plans discarded because the seat was eliminated by an earlier
    /// seat's committed actions in the same turn.
    pub lost_seats: u64,
    /// True if a seat's turn could not be closed within the resolution
    /// bound and the game was abandoned rather than livelocked. Should
    /// never happen; instrumented so it cannot happen silently.
    pub aborted: bool,
}

impl SimultaneousCensus {
    fn note_drop(&mut self, action: &Action) {
        self.dropped += 1;
        *self.dropped_by_kind.entry(kind_name(action)).or_insert(0) += 1;
    }

    /// One line for a run report.
    pub fn summary(&self) -> String {
        let survival = if self.planned == 0 {
            100.0
        } else {
            100.0 * self.applied as f64 / self.planned as f64
        };
        let mut kinds: Vec<(&&str, &u64)> = self.dropped_by_kind.iter().collect();
        kinds.sort_by(|a, b| b.1.cmp(a.1).then(a.0.cmp(b.0)));
        let top: Vec<String> = kinds
            .into_iter()
            .take(3)
            .map(|(kind, count)| format!("{kind} {count}"))
            .collect();
        let mut line = format!(
            "simultaneous: planned {}, applied {} ({survival:.1}%), dropped {}",
            self.planned, self.applied, self.dropped
        );
        if !top.is_empty() {
            line.push_str(&format!(" (top: {})", top.join(", ")));
        }
        if self.forced > 0 {
            line.push_str(&format!(", forced {}", self.forced));
        }
        if self.unplanned_seats > 0 || self.lost_seats > 0 {
            line.push_str(&format!(
                ", seats unplanned {} lost {}",
                self.unplanned_seats, self.lost_seats
            ));
        }
        if self.aborted {
            line.push_str(", ABORTED");
        }
        line
    }
}

/// Play a game out headlessly under whichever turn structure it was set up
/// with. Sequential games go through [`run_game`] unchanged and report no
/// census; simultaneous games report what became of their plans.
pub fn run_structured<A: Ai>(g: &mut Game, ais: &mut [A]) -> Option<SimultaneousCensus> {
    match g.turn_structure {
        TurnStructure::Sequential => {
            run_game(g, ais);
            None
        }
        TurnStructure::Simultaneous => Some(run_simultaneous(g, ais)),
    }
}

/// A seat's planning copy draws from its own turn-keyed stream so parallel
/// speculation can never share draws across seats or shift the game's own
/// serialized stream — the discipline disasters and meteors established.
fn planning_stream(seed: u64, turn: u32, seat: usize) -> Rng {
    Rng::new(seed ^ 0x5349_4D55_4C50_4C41 ^ ((turn as u64) << 20) ^ seat as u64)
}

/// How many mandatory resolutions one seat may need before its `EndTurn`
/// goes through. Each resolution settles one captured city, so the bound is
/// far above anything a real turn produces.
const MANDATORY_RESOLUTION_BOUND: usize = 64;

/// Close `seat`'s turn on `g`, resolving any mandatory choice the plan never
/// made (a captured city's fate) with the first legal answer. Returns false
/// only if the turn could not be closed within the bound — a state the
/// census records and the caller must not spin on.
fn close_seat_turn(g: &mut Game, seat: usize, forced: &mut u64) -> bool {
    for _ in 0..MANDATORY_RESOLUTION_BOUND {
        if g.apply(seat, &Action::EndTurn).is_ok() {
            return true;
        }
        if g.winner.is_some() {
            return true;
        }
        let Some(resolution) = g
            .legal_actions(seat)
            .into_iter()
            .find(|action| !matches!(action, Action::EndTurn))
        else {
            return false;
        };
        if g.apply(seat, &resolution).is_err() {
            return false;
        }
        *forced += 1;
    }
    false
}

fn run_simultaneous<A: Ai>(g: &mut Game, ais: &mut [A]) -> SimultaneousCensus {
    // Same headless observation mode as `run_game`: fog memory is a display
    // cache, not a gameplay input, and planning clones inherit the setting.
    g.set_fog_memory(false);
    let mut census = SimultaneousCensus::default();
    while g.winner.is_none() && g.turn <= g.max_turns {
        // One full cycle of alive seats in the stock cursor order, starting
        // wherever the cursor stands — the same walk `do_end_turn` takes.
        let n = g.players.len();
        let seats: Vec<usize> = (0..n)
            .map(|offset| (g.current + offset) % n)
            .filter(|&seat| g.players[seat].alive)
            .collect();

        // ---- Plan: every seat against the world with nobody having acted.
        // The rolling world takes each seat's EndTurn with no actions, so a
        // later seat plans with its own upkeep applied and its rivals
        // frozen. Each seat then plans on a private copy of that world.
        let mut plans: Vec<(usize, Vec<Action>)> = Vec::with_capacity(seats.len());
        let mut rolling = g.clone();
        let mut forwarded = 0u64;
        for (index, &seat) in seats.iter().enumerate() {
            if index > 0 {
                let previous = seats[index - 1];
                if rolling.winner.is_none() && rolling.current == previous {
                    let _ = close_seat_turn(&mut rolling, previous, &mut forwarded);
                }
            }
            // The forward world can decide the game (a project completing in
            // upkeep) or lose this seat (a loyalty flip taking its last
            // city). Such a seat gets no plan; the live commit decides what
            // actually happens to it.
            if rolling.winner.is_some() || rolling.current != seat {
                census.unplanned_seats += 1;
                plans.push((seat, Vec::new()));
                continue;
            }
            let mut world = rolling.clone();
            world.rng = planning_stream(g.seed, g.turn, seat);
            let mark = world.log.len();
            ais[seat].take_turn(&mut world, seat);
            let actions: Vec<Action> = world
                .log
                .since(mark)
                .take_while(|(pid, _)| *pid == seat)
                .filter(|(_, action)| !matches!(action, Action::EndTurn))
                .map(|(_, action)| action.clone())
                .collect();
            census.planned += actions.len() as u64;
            plans.push((seat, actions));
        }
        drop(rolling);

        // ---- Commit: the plans land in the same seat order, through the
        // ordinary rules, on the one authoritative world. A stale order is
        // dropped; the seat's turn closes through the ordinary EndTurn so
        // upkeep, the world-turn wrap, and victory checks all run exactly
        // where a sequential game runs them.
        for (seat, actions) in plans {
            if g.winner.is_some() {
                break;
            }
            if g.current != seat {
                // An earlier seat's committed actions eliminated this seat;
                // the cursor already skipped it and its plan dies with it.
                census.lost_seats += 1;
                continue;
            }
            for action in &actions {
                if g.winner.is_some() {
                    break;
                }
                match g.apply(seat, action) {
                    Ok(()) => census.applied += 1,
                    Err(_) => census.note_drop(action),
                }
            }
            if g.winner.is_none() && g.current == seat {
                if !close_seat_turn(g, seat, &mut census.forced) {
                    // Closing failed within the bound. Abandon the game
                    // loudly rather than replanning the same turn forever.
                    census.aborted = true;
                    return census;
                }
            }
        }
    }
    census
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::BasicAi;
    use crate::game::GameOptions;

    fn simultaneous_game(seed: u64, turns: u32) -> Game {
        let mut g = Game::new(3, 24, 16, seed, turns, 1);
        g.turn_structure = TurnStructure::Simultaneous;
        g
    }

    #[test]
    fn the_default_structure_is_sequential_and_unchanged() {
        assert_eq!(TurnStructure::default(), TurnStructure::Sequential);
        assert_eq!(
            GameOptions::new(2, 20, 14, 1, 40, 1).turn_structure,
            TurnStructure::Sequential
        );
        // A default game through the structured driver is byte-for-byte the
        // game `run_game` plays, and reports no census.
        let mut direct = Game::new(2, 20, 14, 11, 30, 1);
        let mut ais = BasicAi::fleet(&direct);
        run_game(&mut direct, &mut ais);
        let mut structured = Game::new(2, 20, 14, 11, 30, 1);
        let mut ais = BasicAi::fleet(&structured);
        assert!(run_structured(&mut structured, &mut ais).is_none());
        assert_eq!(
            serde_json::to_value(&direct).unwrap(),
            serde_json::to_value(&structured).unwrap()
        );
    }

    #[test]
    fn a_simultaneous_game_is_deterministic() {
        let mut a = simultaneous_game(9, 40);
        let mut b = simultaneous_game(9, 40);
        let mut ais_a = BasicAi::fleet(&a);
        let mut ais_b = BasicAi::fleet(&b);
        let census_a = run_structured(&mut a, &mut ais_a).expect("a census");
        let census_b = run_structured(&mut b, &mut ais_b).expect("a census");
        assert_eq!(
            serde_json::to_value(&a).unwrap(),
            serde_json::to_value(&b).unwrap()
        );
        assert_eq!(census_a.planned, census_b.planned);
        assert_eq!(census_a.dropped, census_b.dropped);
    }

    /// The structural guarantee of the whole design: a simultaneous game's
    /// log is an ordinary log. Re-applying it through a plain `apply` loop —
    /// no driver, no planning, no snapshots — reproduces the final game
    /// bit-for-bit, exactly as `replay_from_action_log` proves for
    /// sequential games.
    #[test]
    fn a_simultaneous_log_replays_bit_for_bit() {
        let mut g = simultaneous_game(9, 40);
        let mut ais = BasicAi::fleet(&g);
        let census = run_structured(&mut g, &mut ais).expect("a census");
        assert!(!census.aborted);
        assert!(!g.log.is_empty());
        let mut replayed = simultaneous_game(9, 40);
        replayed.set_fog_memory(false);
        for (index, (pid, action)) in g.log.iter().enumerate() {
            replayed.apply(*pid, action).unwrap_or_else(|error| {
                panic!("logged action {index} failed on replay: {error} ({action:?})")
            });
        }
        assert_eq!(
            serde_json::to_value(&g).unwrap(),
            serde_json::to_value(&replayed).unwrap()
        );
    }

    #[test]
    fn the_census_accounts_for_every_planned_action() {
        let mut g = simultaneous_game(21, 40);
        let mut ais = BasicAi::fleet(&g);
        let census = run_structured(&mut g, &mut ais).expect("a census");
        assert!(!census.aborted);
        assert_eq!(census.planned, census.applied + census.dropped);
        assert!(
            g.winner.is_some() || g.turn > g.max_turns,
            "the game must actually finish"
        );
        // The summary line is part of run reports; it must render.
        assert!(census.summary().starts_with("simultaneous: planned"));
    }

    /// The setup choice is part of the game, so it survives a save — and a
    /// save from before the choice existed loads as the sequential game it
    /// was.
    #[test]
    fn turn_structure_survives_a_save_round_trip() {
        let g = simultaneous_game(5, 10);
        let restored: Game =
            serde_json::from_str(&serde_json::to_string(&g).unwrap()).unwrap();
        assert_eq!(restored.turn_structure, TurnStructure::Simultaneous);
        let mut raw: serde_json::Value =
            serde_json::from_str(&serde_json::to_string(&g).unwrap()).unwrap();
        raw.as_object_mut().unwrap().remove("turn_structure");
        let legacy: Game = serde_json::from_value(raw).unwrap();
        assert_eq!(legacy.turn_structure, TurnStructure::Sequential);
    }
}
