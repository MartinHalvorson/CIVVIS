//! Matched skirmish benchmark — the high-power instrument for tactical play.
//!
//! Whole-game win rate is the right acceptance test and the wrong measuring
//! device for this subsystem. `docs/ORACLE.md` puts the resolving power of a
//! 100-cell paired run at roughly a 70/30 split, and a full six-player game
//! produces on the order of fourteen unit kills — so a change that fights
//! meaningfully better can pass through a whole-game A/B leaving no trace, and
//! a change that fights *worse* can too. Measuring the thing a tactical layer
//! actually claims to do needs an instrument aimed at combat itself.
//!
//! This one places two identical armies on the same map, sets them at war, and
//! counts what happens. Every scenario is played **twice with the seats
//! swapped**, so whichever side of the map is better ground, whichever seat
//! moves first, and whatever the terrain does to one army's approach is
//! experienced by both agents in equal measure and cancels in the pair. The
//! reported quantity is the material each agent destroyed minus what it lost,
//! in unit production cost, summed over the two seatings — signed, paired, and
//! one number per seed.
//!
//! Two limits worth stating up front, because they bound what a result here
//! licenses:
//!
//! - **It measures fighting, not winning.** A tactical layer that trades better
//!   and still does not convert that into victories is a real possibility, and
//!   `docs/ORACLE.md` records that military capability granted outright has not
//!   moved wins in this simulator. A gain here is a necessary condition for the
//!   whole-game gate, never a substitute for it.
//! - **The armies are placed, not earned.** Composition is an input, so nothing
//!   here says anything about whether the agent would have built that army.

use std::collections::BTreeMap;

use crate::ai::Ai;
use crate::game::{Action, Game};
use crate::Pos;

/// What one agent did to the other over one scenario.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SkirmishLedger {
    /// Enemy military units destroyed.
    pub kills: usize,
    /// Own military units destroyed.
    pub losses: usize,
    /// Production cost of the enemy units destroyed.
    pub material_destroyed: f64,
    /// Production cost of the own units destroyed.
    pub material_lost: f64,
    /// Hit points taken off enemy units, including the killing blows.
    pub damage_dealt: f64,
    /// Hit points taken off own units.
    pub damage_taken: f64,
}

impl SkirmishLedger {
    fn absorb(&mut self, other: &SkirmishLedger) {
        self.kills += other.kills;
        self.losses += other.losses;
        self.material_destroyed += other.material_destroyed;
        self.material_lost += other.material_lost;
        self.damage_dealt += other.damage_dealt;
        self.damage_taken += other.damage_taken;
    }

    /// The headline: production cost destroyed less production cost lost.
    /// Positive means this agent came out of the engagement ahead.
    pub fn material_swing(&self) -> f64 {
        self.material_destroyed - self.material_lost
    }

    /// Units killed per unit lost. `None` when nothing of this agent's died,
    /// which is a real outcome and must not be flattened into a number.
    pub fn exchange_ratio(&self) -> Option<f64> {
        (self.losses > 0).then(|| self.kills as f64 / self.losses as f64)
    }
}

/// One seed, played twice with the seats swapped.
#[derive(Clone, Debug, Default)]
pub struct MatchedSkirmish {
    pub seed: u64,
    /// Summed over both seatings, so seat and terrain advantage cancel.
    pub a: SkirmishLedger,
    pub b: SkirmishLedger,
    /// Turns actually played, summed over the pair.
    pub turns: u32,
    /// Set when a scenario could not be built — too little land for the
    /// armies, say. Such a seed is reported and excluded, never silently
    /// counted as a draw.
    pub skipped: bool,
}

impl MatchedSkirmish {
    /// A's material swing less B's, over the pair. This is the paired
    /// difference a test should be run on.
    pub fn paired_difference(&self) -> f64 {
        self.a.material_swing() - self.b.material_swing()
    }
}

/// How the armies are built and how long they are given to fight.
#[derive(Clone, Debug)]
pub struct SkirmishSetup {
    pub width: i32,
    pub height: i32,
    /// Unit kinds placed for each side, identically.
    pub army: Vec<String>,
    /// Turns of fighting before the ledger is read.
    pub turns: u32,
    /// Tiles between the two armies' anchor points at the start.
    pub separation: i32,
}

impl Default for SkirmishSetup {
    fn default() -> Self {
        SkirmishSetup {
            width: 28,
            height: 20,
            // A combined-arms band: something to hold the line, something to
            // shoot with, and something fast enough to punish a gap. A pure
            // melee brawl has almost no joint decision to make.
            army: [
                "warrior", "warrior", "spearman", "archer", "archer", "horseman",
            ]
            .iter()
            .map(|kind| kind.to_string())
            .collect(),
            turns: 24,
            separation: 6,
        }
    }
}

/// Play one seed twice with the seats swapped and return the paired ledger.
///
/// `agent` builds a fresh agent by name for a seat — pass
/// [`crate::elo::builtin_ai`].
pub fn matched_skirmish(
    seed: u64,
    setup: &SkirmishSetup,
    name_a: &str,
    name_b: &str,
    agent: &dyn Fn(&str, u64) -> Box<dyn Ai>,
) -> MatchedSkirmish {
    let mut out = MatchedSkirmish {
        seed,
        ..Default::default()
    };
    // Seating 0: A holds seat 0. Seating 1: B does. Same map both times.
    for swapped in [false, true] {
        let seats = if swapped {
            [name_b, name_a]
        } else {
            [name_a, name_b]
        };
        let Some((ledgers, turns)) = play_skirmish(seed, setup, seats, agent) else {
            out.skipped = true;
            return out;
        };
        let (first, second) = ledgers;
        if swapped {
            out.b.absorb(&first);
            out.a.absorb(&second);
        } else {
            out.a.absorb(&first);
            out.b.absorb(&second);
        }
        out.turns += turns;
    }
    out
}

/// Run one scenario. Returns the two seats' ledgers in seat order, or `None`
/// when the map could not seat the armies.
fn play_skirmish(
    seed: u64,
    setup: &SkirmishSetup,
    seats: [&str; 2],
    agent: &dyn Fn(&str, u64) -> Box<dyn Ai>,
) -> Option<((SkirmishLedger, SkirmishLedger), u32)> {
    let mut game = build_scenario(seed, setup)?;
    let mut ais: Vec<Box<dyn Ai>> = (0..2)
        .map(|pid| agent(seats[pid], seed.wrapping_add(pid as u64)))
        .collect();

    let mut ledgers = (SkirmishLedger::default(), SkirmishLedger::default());
    let mut previous = snapshot(&game);
    let start = game.turn;
    let deadline = start + setup.turns;

    while game.winner.is_none() && game.turn < deadline {
        let pid = game.current;
        if pid < 2 {
            ais[pid].take_turn(&mut game, pid);
        }
        if game.winner.is_none() && game.current == pid {
            let _ = game.apply(pid, &Action::EndTurn);
        }
        let now = snapshot(&game);
        account(&previous, &now, &game, &mut ledgers);
        previous = now;
        // Nothing left to measure once a side has no army in the field.
        if [0usize, 1]
            .iter()
            .any(|side| !previous.values().any(|unit| unit.0 == *side))
        {
            break;
        }
    }
    Some((ledgers, game.turn.saturating_sub(start)))
}

/// uid -> (owner, kind, hp) for every military unit belonging to a combatant.
type Snapshot = BTreeMap<u32, (usize, String, i32)>;

fn snapshot(g: &Game) -> Snapshot {
    g.units
        .values()
        .filter(|unit| unit.owner < 2 && g.rules.units[unit.kind].class == "military")
        .map(|unit| (unit.id, (unit.owner, unit.kind.to_string(), unit.hp)))
        .collect()
}

/// Fold one turn's change into both seats' ledgers.
///
/// A unit that vanished is counted as destroyed. Over a scenario this short,
/// with no gold to upgrade with and Nationalism many civics away, disappearing
/// is combat death; the alternatives are unreachable here.
fn account(
    before: &Snapshot,
    after: &Snapshot,
    g: &Game,
    ledgers: &mut (SkirmishLedger, SkirmishLedger),
) {
    for (uid, (owner, kind, hp)) in before {
        let side = *owner;
        match after.get(uid) {
            None => {
                let cost = g.rules.units.get(kind.as_str()).map_or(0.0, |spec| spec.cost);
                let taken = *hp as f64;
                let (mine, theirs) = split(ledgers, side);
                mine.losses += 1;
                mine.material_lost += cost;
                mine.damage_taken += taken;
                theirs.kills += 1;
                theirs.material_destroyed += cost;
                theirs.damage_dealt += taken;
            }
            Some((_, _, now)) => {
                let taken = (*hp - *now).max(0) as f64;
                if taken > 0.0 {
                    let (mine, theirs) = split(ledgers, side);
                    mine.damage_taken += taken;
                    theirs.damage_dealt += taken;
                }
            }
        }
    }
}

/// Borrow `side`'s ledger and its opponent's at once.
fn split(
    ledgers: &mut (SkirmishLedger, SkirmishLedger),
    side: usize,
) -> (&mut SkirmishLedger, &mut SkirmishLedger) {
    if side == 0 {
        (&mut ledgers.0, &mut ledgers.1)
    } else {
        (&mut ledgers.1, &mut ledgers.0)
    }
}

/// Found both capitals from the ordinary start, seat two identical armies
/// facing each other between them, and declare war.
fn build_scenario(seed: u64, setup: &SkirmishSetup) -> Option<Game> {
    let mut g = Game::new_full(
        2,
        setup.width,
        setup.height,
        seed,
        setup.turns + 8,
        0,
        false,
    );

    // Take the normal opening so both sides have a city to heal at and a
    // production queue, then stop; everything after this is the battle.
    for pid in 0..2 {
        let settler = g
            .player_unit_ids(pid)
            .into_iter()
            .find(|uid| g.units[uid].kind == "settler")?;
        g.apply(pid, &Action::FoundCity { unit: settler }).ok()?;
        g.apply(pid, &Action::EndTurn).ok()?;
    }

    let home: Vec<Pos> = (0..2)
        .map(|pid| g.cities[&g.player_city_ids(pid)[0]].pos)
        .collect();
    // Each army musters on its own side of the midpoint, so the two forces
    // start `separation` apart and have to close.
    let midpoint = (
        (home[0].0 + home[1].0) / 2,
        (home[0].1 + home[1].1) / 2,
    );
    let half = (setup.separation / 2).max(1);
    let anchors: Vec<Pos> = (0..2)
        .map(|pid| toward(&g, midpoint, home[pid], half))
        .collect();

    for pid in 0..2 {
        if !place_army(&mut g, pid, &setup.army, anchors[pid]) {
            return None;
        }
    }

    g.record_contact(0, 1);
    g.apply(0, &Action::DeclareWar { player: 1 }).ok()?;
    Some(g)
}

/// The tile `steps` along the way from `from` toward `target`, staying on land.
fn toward(g: &Game, from: Pos, target: Pos, steps: i32) -> Pos {
    let mut at = from;
    for _ in 0..steps {
        let next = g
            .nbrs(at)
            .into_iter()
            .filter(|pos| g.map.get(*pos).is_some_and(|tile| !g.rules.is_water(tile)))
            .min_by_key(|pos| (g.wdist(*pos, target), *pos));
        match next {
            Some(pos) if g.wdist(pos, target) < g.wdist(at, target) => at = pos,
            _ => break,
        }
    }
    at
}

/// Seat every unit of `army` on dry, empty, city-free ground near `anchor`.
/// Returns false if the map cannot take the whole army, so the caller can drop
/// the seed rather than measure a half-placed force.
fn place_army(g: &mut Game, pid: usize, army: &[String], anchor: Pos) -> bool {
    let mut placed = 0usize;
    for radius in 0..=4 {
        for pos in g.wdisk(anchor, radius) {
            if placed == army.len() {
                return true;
            }
            let usable = g
                .map
                .get(pos)
                .is_some_and(|tile| !g.rules.is_water(tile) && g.rules.is_passable(tile));
            if !usable || g.city_at(pos).is_some() || !g.units_at(pos).is_empty() {
                continue;
            }
            g.spawn_unit(&army[placed], pid, pos);
            placed += 1;
        }
    }
    placed == army.len()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::elo::builtin_ai;

    /// The instrument has to move before it can measure anything. A scenario
    /// that ends with both armies untouched would report a null for every
    /// treatment, and it would be the harness saying it, not the agents.
    #[test]
    fn a_skirmish_actually_produces_a_fight() {
        let setup = SkirmishSetup::default();
        let result = matched_skirmish(4_100, &setup, "advanced", "advanced", &builtin_ai);
        assert!(!result.skipped, "seed 4100 must seat both armies");
        let blows = result.a.damage_dealt + result.b.damage_dealt;
        assert!(
            blows > 0.0,
            "no damage was dealt in {} turns of a declared war",
            result.turns
        );
    }

    /// Both seats run the same agent, so anything the pair reports as a
    /// difference is coming from the map or the seat — exactly what swapping
    /// the seats is supposed to remove. Damage is conserved: what one side
    /// dealt, the other took.
    #[test]
    fn the_ledger_is_conserved_between_the_two_sides() {
        let setup = SkirmishSetup::default();
        let result = matched_skirmish(4_101, &setup, "advanced", "advanced", &builtin_ai);
        assert!(!result.skipped);
        assert_eq!(result.a.kills, result.b.losses);
        assert_eq!(result.b.kills, result.a.losses);
        assert!((result.a.damage_dealt - result.b.damage_taken).abs() < 1e-9);
        assert!((result.a.material_destroyed - result.b.material_lost).abs() < 1e-9);
    }

    /// Bookkeeping check on the swap. With one agent in both seats the two
    /// seatings play out identically, so `a` and `b` must receive the same
    /// pair of ledgers in opposite order and net to zero. This is what catches
    /// a swap that credits the wrong agent — the failure mode that would make
    /// every treatment look like it won.
    #[test]
    fn an_agent_against_itself_nets_to_zero_over_the_swap() {
        let setup = SkirmishSetup::default();
        for seed in [4_102u64, 4_103, 4_104] {
            let result = matched_skirmish(seed, &setup, "advanced", "advanced", &builtin_ai);
            if result.skipped {
                continue;
            }
            assert!(
                result.paired_difference().abs() < 1e-9,
                "seed {seed} gave a self-match a paired difference of {}",
                result.paired_difference()
            );
        }
    }
}
