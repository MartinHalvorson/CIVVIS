//! The doctrine arena: hand-built tactical positions, and an instrument that
//! names *how* an agent fought.
//!
//! `src/skirmish.rs` measures whether one tactical agent trades better than
//! another. It does that well and it is the right gate. What it cannot do is
//! say *why*, and it cannot pose a problem: two identical armies dropped six
//! tiles apart in open ground is one tactical question asked over and over,
//! and it is the easiest one there is. An agent can be excellent at the
//! stand-up fight and have no idea what to do when the enemy arrives in two
//! separate columns, when the ground funnels, or when it is holding a ridge
//! against something that has to climb it.
//!
//! This module poses the other questions. Each [`Position`] is a board painted
//! tile by tile and a force deployed unit by unit, taken from an engagement
//! that made a general's reputation, and reduced to the one decision that
//! engagement turned on:
//!
//! - **Central position** (Bonaparte, Montenotte 1796; Ligny–Quatre Bras 1815)
//!   — the enemy comes in two columns that have to unite. Beat one before the
//!   other arrives, or be beaten by both.
//! - **Oblique order** (Epaminondas at Leuctra, 371 BC; Frederick at Leuthen,
//!   1757) — equal numbers, unequal deployment. One side has massed a wing and
//!   refused the other. Weight tells if it is used before the thin flank folds.
//! - **The defile** (Leonidas at Thermopylae; Bonaparte at the Arcole
//!   causeway) — a wall with one gap. The small force holds the gap; the large
//!   one cannot bring its numbers to bear through it and must find another way
//!   or pay for the frontal attempt.
//! - **The ridge** (Wellington at Waterloo) — one side has the missiles, the
//!   other has to close. Ground decides which of those facts matters.
//! - **Double envelopment** (Hannibal at Cannae, 216 BC) — fewer but better
//!   troops against a dense mass. Give ground in the centre, come round the
//!   wings, or be ground down.
//! - **The reserve** (Bonaparte at Marengo; the Guard at Waterloo) — half the
//!   army is in contact and half is behind it. Reserves win engagements when
//!   they arrive together and lose them when they are fed in one at a time.
//! - **The river line** (Bonaparte at Lodi 1796; Friedland 1807) — an obstacle
//!   with two crossings. An army caught astride an obstacle is two armies, and
//!   an army with one at its back has nowhere to go.
//!
//! ## Why the board is painted rather than generated
//!
//! A generated map is a *sample*; a position is a *fact*. Every tile of every
//! position here is written down, so the defile is a defile in every run, on
//! every machine, and after every change to map generation. Only the arena's
//! topology and its rules come from the engine — the terrain does not. What
//! varies between seeds is the **muster**: each unit is nudged to a nearby free
//! tile by a seeded jitter, so a position yields independent samples without
//! ever stopping being that position. A doctrine that only works from an exact
//! deployment is not a doctrine.
//!
//! ## The pairing, and what the control must report
//!
//! A position is asymmetric on purpose — that is the whole point of posing a
//! problem — so a single playing says nothing about the two agents. Each seed
//! is therefore played **twice with the roles swapped**: A takes the first side
//! and B the second, then B takes the first and A the second. Each agent
//! experiences both sides of the problem in equal measure, and the reported
//! quantity is the sum over the pair.
//!
//! As in `skirmish.rs`, the same agent in both roles must net to **exactly
//! zero** on every seed, and [`a_self_match_nets_to_zero_on_every_position`]
//! asserts it. That is what licenses reading any treatment number out of this
//! harness.
//!
//! ## What the doctrine profile is, and what it is not
//!
//! Material swing says who won the trade. The [`DoctrineProfile`] says how they
//! fought, from the board alone — concentration at the point of contact,
//! whether the army moved as one body or as scattered pieces, whether enemies
//! were taken from more than one side, whether fire was concentrated on units
//! that died or spread across units that lived, and what ground was held.
//!
//! These are **descriptions, not scores**. Nothing here adds them up into a
//! quality number, because the whole content of a doctrine is that the right
//! value depends on the position: an army holding a defile *should* be dense
//! and static, and the same numbers from an army that was supposed to envelop
//! mean it failed. The profile is for reading beside a swing, to say what a
//! change to the tactical layer actually changed about its behaviour.

use std::collections::BTreeMap;

use crate::ai::Ai;
use crate::game::Game;
use crate::hex;
use crate::rng::Rng;
use crate::setup::{GameSpeed, MapScript};
use crate::Pos;

/// One unit placed on an offset (column, row) cell of the position's board.
#[derive(Clone, Copy, Debug)]
pub struct Deploy {
    pub kind: &'static str,
    pub col: i32,
    pub row: i32,
}

/// Shorthand so a deployment reads as a list of placements rather than a wall
/// of struct literals.
const fn at(kind: &'static str, col: i32, row: i32) -> Deploy {
    Deploy { kind, col, row }
}

/// What a brush paints onto a cell. The base of every board is flat grassland,
/// so a position only writes what makes it that position.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Brush {
    /// Impassable. The wall of a defile, the crest that cannot be crossed.
    Mountain,
    /// Passable, defensible, and slow. High ground.
    Hills,
    /// Cover: a defence bonus without the height.
    Forest,
    /// Impassable to land units. A river line, a lake, the sea on a flank.
    Water,
    /// Open but poor going.
    Marsh,
}

/// A brush and the cells it is applied to, in offset (column, row).
pub type Stroke = (Brush, &'static [(i32, i32)]);

/// A hand-built tactical position: a board, two deployments, and the doctrine
/// the engagement it comes from turned on.
#[derive(Clone, Copy, Debug)]
pub struct Position {
    /// Stable identifier, used on the command line and in reports.
    pub id: &'static str,
    pub name: &'static str,
    /// The general and the engagement.
    pub provenance: &'static str,
    /// The tactical problem, in one sentence.
    pub problem: &'static str,
    /// What each side is trying to do, in role order.
    pub roles: [&'static str; 2],
    pub width: i32,
    pub height: i32,
    /// Turns of fighting before the ledger is read.
    pub turns: u32,
    pub terrain: &'static [Stroke],
    /// Forces in role order. Deliberately allowed to be unequal — an even
    /// fight is one tactical problem out of many, and rarely the interesting
    /// one.
    pub forces: [&'static [Deploy]; 2],
}

impl Position {
    /// Total production cost of one side's force. Reported beside a result
    /// because most of these positions are deliberately unequal, and a swing
    /// means something different at 240 material than at 600.
    pub fn material(&self, role: usize, rules: &crate::rules::Rules) -> f64 {
        self.forces[role]
            .iter()
            .map(|deploy| rules.units.get(deploy.kind).map_or(0.0, |spec| spec.cost))
            .sum()
    }
}

/// Every position in the arena, in the order they are reported.
pub const POSITIONS: &[Position] = &[
    Position {
        id: "central_position",
        name: "The central position",
        provenance: "Bonaparte, Montenotte 1796 and Ligny-Quatre Bras 1815",
        problem: "The enemy arrives as two columns that must unite. Beat one \
                  before the other reaches it, or be beaten by both.",
        roles: [
            "interior lines: one body between two, with the shorter march",
            "converging wings: two bodies that are stronger only together",
        ],
        width: 30,
        height: 15,
        turns: 26,
        // A belt of high ground down the middle: the interior force has the
        // shorter road across it, which is the whole advantage of the position.
        terrain: &[
            (
                Brush::Hills,
                &[
                    (14, 4),
                    (15, 5),
                    (14, 6),
                    (15, 7),
                    (14, 8),
                    (15, 9),
                    (14, 10),
                ],
            ),
            (Brush::Forest, &[(13, 3), (16, 11), (13, 11), (16, 3)]),
        ],
        forces: [
            &[
                at("warrior", 15, 6),
                at("warrior", 14, 7),
                at("spearman", 15, 8),
                at("archer", 16, 6),
                at("archer", 16, 8),
                at("horseman", 16, 7),
            ],
            &[
                at("warrior", 3, 6),
                at("spearman", 3, 8),
                at("archer", 2, 7),
                at("warrior", 26, 6),
                at("spearman", 26, 8),
                at("archer", 27, 7),
            ],
        ],
    },
    Position {
        id: "oblique_order",
        name: "The oblique order",
        provenance: "Epaminondas at Leuctra 371 BC; Frederick at Leuthen 1757",
        problem: "Equal numbers, unequal deployment: one side has massed a \
                  wing and refused the other. Weight tells only if it is spent \
                  before the thin flank folds.",
        roles: [
            "the weighted wing: strong left, refused right",
            "the even line: no weak point and no strong one",
        ],
        width: 22,
        height: 18,
        turns: 24,
        terrain: &[
            // A marsh anchoring the southern flank, so the weighted wing has
            // somewhere its refused flank can lean.
            (Brush::Marsh, &[(9, 15), (10, 16), (11, 15), (10, 14)]),
            (Brush::Forest, &[(6, 2), (7, 3), (14, 15), (15, 14)]),
        ],
        forces: [
            &[
                // Four on the northern wing, two refused to the south.
                at("swordsman", 6, 4),
                at("swordsman", 7, 4),
                at("spearman", 6, 5),
                at("archer", 7, 5),
                at("warrior", 7, 12),
                at("archer", 6, 12),
            ],
            &[
                at("spearman", 15, 3),
                at("warrior", 15, 6),
                at("archer", 14, 7),
                at("spearman", 15, 9),
                at("warrior", 15, 12),
                at("archer", 14, 11),
            ],
        ],
    },
    Position {
        id: "the_defile",
        name: "The defile",
        provenance: "Leonidas at Thermopylae 480 BC; Bonaparte at the Arcole causeway",
        problem: "A wall with one gap. The small force holds it; the large \
                  force cannot bring its numbers through it.",
        roles: [
            "the stopper: three units and a two-tile front",
            "the column: seven units and one way in",
        ],
        width: 24,
        height: 13,
        turns: 26,
        terrain: &[
            (
                Brush::Mountain,
                &[
                    (12, 0),
                    (12, 1),
                    (12, 2),
                    (12, 3),
                    (12, 4),
                    (12, 8),
                    (12, 9),
                    (12, 10),
                    (12, 11),
                    (12, 12),
                    (11, 0),
                    (11, 1),
                    (11, 2),
                    (11, 11),
                    (11, 12),
                    (13, 0),
                    (13, 12),
                ],
            ),
            // The gap itself is high ground: holding it is worth something,
            // and taking it costs.
            (Brush::Hills, &[(12, 5), (12, 6), (12, 7)]),
        ],
        forces: [
            &[
                at("spearman", 13, 5),
                at("spearman", 13, 7),
                at("archer", 14, 6),
            ],
            &[
                at("warrior", 8, 5),
                at("warrior", 8, 7),
                at("swordsman", 8, 6),
                at("swordsman", 7, 5),
                at("archer", 7, 7),
                at("archer", 6, 6),
                at("horseman", 6, 4),
            ],
        ],
    },
    Position {
        id: "the_ridge",
        name: "The ridge",
        provenance: "Wellington at Waterloo, 1815",
        problem: "One side has the missiles and the ground; the other has to \
                  climb. Which of those facts decides it is a tactical \
                  question, not an arithmetic one.",
        roles: [
            "the defence: ranged behind high ground",
            "the assault: shock troops that must close",
        ],
        width: 22,
        height: 16,
        turns: 24,
        terrain: &[
            (
                Brush::Hills,
                &[
                    (11, 2),
                    (11, 3),
                    (12, 4),
                    (11, 5),
                    (12, 6),
                    (11, 7),
                    (12, 8),
                    (11, 9),
                    (12, 10),
                    (11, 11),
                    (11, 12),
                    (11, 13),
                ],
            ),
            // Hougoumont and La Haye Sainte: two woods in front of the ridge
            // that break up an approach made without thought.
            (Brush::Forest, &[(8, 4), (8, 5), (8, 10), (8, 11)]),
        ],
        forces: [
            &[
                at("archer", 13, 5),
                at("archer", 13, 7),
                at("archer", 13, 9),
                at("archer", 14, 6),
                at("spearman", 12, 5),
                at("spearman", 12, 9),
            ],
            &[
                at("swordsman", 4, 6),
                at("swordsman", 4, 8),
                at("warrior", 3, 5),
                at("warrior", 3, 9),
                at("warrior", 3, 7),
                at("horseman", 2, 7),
            ],
        ],
    },
    Position {
        id: "double_envelopment",
        name: "Double envelopment",
        provenance: "Hannibal at Cannae, 216 BC",
        problem: "Fewer but better troops against a dense mass. Give ground \
                  in the centre and come round the wings, or be ground down \
                  by weight of numbers.",
        roles: [
            "the mass: nine cheap units in depth",
            "the wings: five better ones, two of them fast",
        ],
        width: 24,
        height: 16,
        turns: 24,
        terrain: &[
            // The Aufidus: an open flank on one side only, so an envelopment
            // has one wing to come round rather than two easy ones.
            (Brush::Water, &[(10, 0), (11, 0), (12, 0), (13, 0), (11, 1)]),
            (Brush::Forest, &[(6, 13), (17, 13)]),
        ],
        forces: [
            &[
                at("warrior", 7, 5),
                at("warrior", 7, 6),
                at("warrior", 7, 7),
                at("warrior", 7, 8),
                at("warrior", 6, 5),
                at("warrior", 6, 7),
                at("slinger", 6, 6),
                at("slinger", 6, 8),
                at("slinger", 5, 7),
            ],
            &[
                at("spearman", 16, 6),
                at("spearman", 16, 8),
                at("swordsman", 16, 7),
                at("horseman", 17, 4),
                at("horseman", 17, 10),
            ],
        ],
    },
    Position {
        id: "the_reserve",
        name: "The reserve",
        provenance: "Bonaparte at Marengo 1800; the Guard at Waterloo 1815",
        problem: "Half the army is in contact and half is six tiles behind \
                  it. A reserve wins an engagement when it arrives together \
                  and loses one when it is fed in a unit at a time.",
        roles: [
            "the near reserve: four in contact, two close behind",
            "the far reserve: three in contact, three further back",
        ],
        width: 24,
        height: 14,
        turns: 26,
        terrain: &[
            (Brush::Forest, &[(11, 2), (12, 2), (11, 11), (12, 11)]),
            (Brush::Hills, &[(12, 6), (11, 7)]),
        ],
        forces: [
            &[
                at("warrior", 9, 5),
                at("spearman", 9, 7),
                at("archer", 8, 6),
                at("swordsman", 9, 6),
                at("archer", 4, 6),
                at("horseman", 4, 7),
            ],
            &[
                at("warrior", 14, 5),
                at("spearman", 14, 7),
                at("swordsman", 14, 6),
                at("archer", 20, 5),
                at("archer", 20, 7),
                at("horseman", 21, 6),
            ],
        ],
    },
    Position {
        id: "the_river_line",
        name: "The river line",
        provenance: "Bonaparte at Lodi 1796 and Friedland 1807",
        problem: "An obstacle with two crossings. An army astride one is two \
                  armies; an army with one at its back has nowhere to go.",
        roles: [
            "the near bank: closer to both fords",
            "the far bank: must cross, or make the other side cross",
        ],
        width: 26,
        height: 16,
        turns: 26,
        terrain: &[
            (
                Brush::Water,
                &[
                    (13, 0),
                    (13, 1),
                    (13, 2),
                    (13, 3),
                    (13, 4),
                    (13, 6),
                    (13, 7),
                    (13, 8),
                    (13, 9),
                    (13, 11),
                    (13, 12),
                    (13, 13),
                    (13, 14),
                    (13, 15),
                    (12, 1),
                    (12, 3),
                    (12, 8),
                    (12, 13),
                ],
            ),
            // The two fords, and high ground overlooking each.
            (Brush::Hills, &[(14, 5), (14, 10), (12, 5), (12, 10)]),
        ],
        forces: [
            &[
                at("warrior", 10, 5),
                at("spearman", 10, 10),
                at("archer", 9, 6),
                at("archer", 9, 9),
                at("swordsman", 9, 7),
                at("horseman", 8, 8),
            ],
            &[
                at("warrior", 17, 5),
                at("spearman", 17, 10),
                at("archer", 18, 6),
                at("archer", 18, 9),
                at("swordsman", 18, 7),
                at("horseman", 19, 8),
            ],
        ],
    },
];

/// Look a position up by its identifier.
pub fn position(id: &str) -> Option<&'static Position> {
    POSITIONS.iter().find(|spec| spec.id == id)
}

/// What one side did to the other, and how it went about it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct DoctrineLedger {
    pub kills: usize,
    pub losses: usize,
    pub material_destroyed: f64,
    pub material_lost: f64,
    pub damage_dealt: f64,
    /// Damage dealt to enemy units that went on to die. The rest was spent on
    /// units that survived the engagement.
    pub damage_on_the_dead: f64,
    pub damage_taken: f64,
    /// Board observations, accumulated turn by turn; see [`DoctrineProfile`].
    observations: Observations,
}

/// Running sums behind [`DoctrineProfile`]. Kept private so the profile is the
/// only way to read them and the normalisation cannot be got wrong by a caller.
#[derive(Clone, Debug, Default, PartialEq)]
struct Observations {
    /// Turns observed at all.
    turns: usize,
    /// Turns on which any of this side's units stood within two tiles of an
    /// enemy — the only turns on which a contact statistic means anything.
    contact_turns: usize,
    /// Summed over contact turns: own units within two tiles of the contact
    /// zone, less enemy units within two tiles of it.
    local_ratio: f64,
    /// Summed over turns: mean pairwise distance between own units.
    dispersion: f64,
    /// Summed over contact turns: enemy units adjacent to two or more of ours.
    enveloped: f64,
    /// Summed over turns: own units standing on defensible ground.
    on_good_ground: f64,
    /// Summed over turns: own units standing where a friendly unit is between
    /// them and the nearest enemy.
    screened_ranged: f64,
    /// Summed over turns: own ranged units alive, the denominator for the
    /// screen figure.
    ranged_alive: f64,
    /// Summed over turns: own units alive, the denominator for the rest.
    alive: f64,
}

impl DoctrineLedger {
    fn absorb(&mut self, other: &DoctrineLedger) {
        self.kills += other.kills;
        self.losses += other.losses;
        self.material_destroyed += other.material_destroyed;
        self.material_lost += other.material_lost;
        self.damage_dealt += other.damage_dealt;
        self.damage_on_the_dead += other.damage_on_the_dead;
        self.damage_taken += other.damage_taken;
        let mine = &mut self.observations;
        let theirs = &other.observations;
        mine.turns += theirs.turns;
        mine.contact_turns += theirs.contact_turns;
        mine.local_ratio += theirs.local_ratio;
        mine.dispersion += theirs.dispersion;
        mine.enveloped += theirs.enveloped;
        mine.on_good_ground += theirs.on_good_ground;
        mine.screened_ranged += theirs.screened_ranged;
        mine.ranged_alive += theirs.ranged_alive;
        mine.alive += theirs.alive;
    }

    /// Production cost destroyed less production cost lost.
    pub fn material_swing(&self) -> f64 {
        self.material_destroyed - self.material_lost
    }

    /// How the fighting was done, normalised out of the running sums.
    pub fn profile(&self) -> DoctrineProfile {
        let obs = &self.observations;
        let per_turn = |total: f64| (obs.turns > 0).then(|| total / obs.turns as f64);
        let per_contact = |total: f64| (obs.contact_turns > 0).then(|| total / obs.contact_turns as f64);
        DoctrineProfile {
            concentration: per_contact(obs.local_ratio),
            dispersion: per_turn(obs.dispersion),
            envelopment: per_contact(obs.enveloped),
            focus: (self.damage_dealt > 0.0).then(|| self.damage_on_the_dead / self.damage_dealt),
            ground: (obs.alive > 0.0).then(|| obs.on_good_ground / obs.alive),
            screen: (obs.ranged_alive > 0.0).then(|| obs.screened_ranged / obs.ranged_alive),
            contact: per_turn(obs.contact_turns as f64),
        }
    }
}

/// How a side fought, read off the board rather than off the result.
///
/// Every field is `None` when the engagement gave nothing to measure it from —
/// a side that never came within two tiles of an enemy has no concentration at
/// contact, and reporting a zero there would be the harness inventing a fact.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct DoctrineProfile {
    /// Own units near the point of contact less enemy units near it, averaged
    /// over the turns there was contact. Positive is local superiority — the
    /// thing every general on this list was actually trying to arrange.
    pub concentration: Option<f64>,
    /// Mean pairwise distance between own units, averaged over turns. Low is
    /// a body that moves together; high is one that has come apart.
    pub dispersion: Option<f64>,
    /// Enemy units taken from two or more sides at once, per contact turn.
    pub envelopment: Option<f64>,
    /// Share of damage dealt that landed on enemies that died. High is fire
    /// concentrated until the target went down; low is damage spread across a
    /// front that healed it off.
    pub focus: Option<f64>,
    /// Share of own unit-turns spent on defensible ground.
    pub ground: Option<f64>,
    /// Share of own ranged unit-turns spent with a friendly unit between them
    /// and the nearest enemy.
    pub screen: Option<f64>,
    /// Share of turns on which contact existed at all. A low figure on a
    /// position that has to be attacked is an agent that declined to fight.
    pub contact: Option<f64>,
}

/// One position, one seed, played twice with the roles swapped.
#[derive(Clone, Debug, Default)]
pub struct MatchedPosition {
    pub seed: u64,
    /// Summed over both role assignments, so the position's own asymmetry
    /// cancels within the pair.
    pub a: DoctrineLedger,
    pub b: DoctrineLedger,
    /// Each agent's ledger in the first role and in the second, so a report
    /// can say which side of the problem an agent is better at.
    pub a_by_role: [DoctrineLedger; 2],
    pub b_by_role: [DoctrineLedger; 2],
    pub turns: u32,
    /// Set when the board could not seat both forces. Reported and excluded,
    /// never counted as a draw.
    pub skipped: bool,
}

impl MatchedPosition {
    /// A's material swing less B's, over the pair — the paired difference a
    /// test should be run on.
    pub fn paired_difference(&self) -> f64 {
        self.a.material_swing() - self.b.material_swing()
    }
}

/// Play one position on one seed, twice, with the two agents swapping roles.
///
/// `agent` builds a fresh agent by name for a seat — pass
/// [`crate::elo::builtin_ai`].
pub fn matched_position(
    spec: &Position,
    seed: u64,
    name_a: &str,
    name_b: &str,
    agent: &dyn Fn(&str, u64) -> Box<dyn Ai>,
) -> MatchedPosition {
    let mut out = MatchedPosition {
        seed,
        ..Default::default()
    };
    for swapped in [false, true] {
        let seats = if swapped {
            [name_b, name_a]
        } else {
            [name_a, name_b]
        };
        let Some((ledgers, turns)) = play_position(spec, seed, seats, agent) else {
            out.skipped = true;
            return out;
        };
        let (first, second) = ledgers;
        if swapped {
            out.b.absorb(&first);
            out.a.absorb(&second);
            out.b_by_role[0] = first;
            out.a_by_role[1] = second;
        } else {
            out.a.absorb(&first);
            out.b.absorb(&second);
            out.a_by_role[0] = first;
            out.b_by_role[1] = second;
        }
        out.turns += turns;
    }
    out
}

/// A unit as the instrument sees it: who owns it, what it is, how hurt it is,
/// and where it stands.
#[derive(Clone, Debug, PartialEq)]
struct Seen {
    owner: usize,
    kind: String,
    hp: i32,
    pos: Pos,
    ranged: bool,
}

type Snapshot = BTreeMap<u32, Seen>;

/// Run one position with the given agents in role order. Returns both roles'
/// ledgers and the turns played, or `None` when the board could not seat the
/// forces.
fn play_position(
    spec: &Position,
    seed: u64,
    seats: [&str; 2],
    agent: &dyn Fn(&str, u64) -> Box<dyn Ai>,
) -> Option<((DoctrineLedger, DoctrineLedger), u32)> {
    let mut game = build(spec, seed)?;
    let mut ais: Vec<Box<dyn Ai>> = (0..2)
        .map(|pid| agent(seats[pid], seed.wrapping_add(pid as u64)))
        .collect();

    let mut ledgers = (DoctrineLedger::default(), DoctrineLedger::default());
    let mut previous = snapshot(&game);
    let start = game.turn;
    let deadline = start + spec.turns;
    // Damage is attributed as it is dealt, but whether it was spent well is
    // only known once the target's fate is: hold it per victim and settle up
    // when the unit dies or the engagement ends.
    let mut pending: BTreeMap<u32, f64> = BTreeMap::new();

    observe(&previous, &game, &mut ledgers);
    while game.winner.is_none() && game.turn < deadline {
        let pid = game.current;
        if pid < 2 {
            ais[pid].take_turn(&mut game, pid);
        }
        if game.winner.is_none() && game.current == pid {
            let _ = game.apply(pid, &crate::game::Action::EndTurn);
        }
        let now = snapshot(&game);
        account(&previous, &now, &game, &mut ledgers, &mut pending);
        previous = now;
        observe(&previous, &game, &mut ledgers);
        // Nothing left to measure once a side has no unit standing.
        if [0usize, 1]
            .iter()
            .any(|side| !previous.values().any(|unit| unit.owner == *side))
        {
            break;
        }
    }
    Some((ledgers, game.turn.saturating_sub(start)))
}

fn snapshot(g: &Game) -> Snapshot {
    g.units
        .values()
        .filter(|unit| unit.owner < 2 && g.rules.units[unit.kind].class == "military")
        .map(|unit| {
            let ranged = g
                .rules
                .units
                .get(&unit.kind)
                .is_some_and(|spec| spec.range > 0);
            (
                unit.id,
                Seen {
                    owner: unit.owner,
                    kind: unit.kind.to_string(),
                    hp: unit.hp,
                    pos: unit.pos,
                    ranged,
                },
            )
        })
        .collect()
}

/// Fold one turn's change into both ledgers.
fn account(
    before: &Snapshot,
    after: &Snapshot,
    g: &Game,
    ledgers: &mut (DoctrineLedger, DoctrineLedger),
    pending: &mut BTreeMap<u32, f64>,
) {
    for (uid, seen) in before {
        let side = seen.owner;
        match after.get(uid) {
            None => {
                let cost = g
                    .rules
                    .units
                    .get(seen.kind.as_str())
                    .map_or(0.0, |spec| spec.cost);
                let final_blow = seen.hp as f64;
                let earlier = pending.remove(uid).unwrap_or(0.0);
                let (mine, theirs) = split(ledgers, side);
                mine.losses += 1;
                mine.material_lost += cost;
                mine.damage_taken += final_blow;
                theirs.kills += 1;
                theirs.material_destroyed += cost;
                theirs.damage_dealt += final_blow;
                // Everything that went into this unit was well spent.
                theirs.damage_on_the_dead += earlier + final_blow;
            }
            Some(now) => {
                let taken = (seen.hp - now.hp).max(0) as f64;
                if taken > 0.0 {
                    let (mine, theirs) = split(ledgers, side);
                    mine.damage_taken += taken;
                    theirs.damage_dealt += taken;
                    *pending.entry(*uid).or_default() += taken;
                }
            }
        }
    }
}

/// Read the board and fold one turn of doctrine observations into both sides.
fn observe(now: &Snapshot, g: &Game, ledgers: &mut (DoctrineLedger, DoctrineLedger)) {
    for side in [0usize, 1] {
        let mine: Vec<&Seen> = now.values().filter(|unit| unit.owner == side).collect();
        let theirs: Vec<&Seen> = now.values().filter(|unit| unit.owner != side).collect();
        if mine.is_empty() || theirs.is_empty() {
            continue;
        }
        let (ledger, _) = split(ledgers, side);
        let obs = &mut ledger.observations;
        obs.turns += 1;
        obs.alive += mine.len() as f64;

        // Dispersion: how far apart this side's own units are from each other.
        let mut pairs = 0usize;
        let mut total = 0f64;
        for (index, unit) in mine.iter().enumerate() {
            for other in mine.iter().skip(index + 1) {
                total += f64::from(g.wdist(unit.pos, other.pos));
                pairs += 1;
            }
        }
        if pairs > 0 {
            obs.dispersion += total / pairs as f64;
        }

        // Ground and screen are per-unit facts about where this side stands.
        for unit in &mine {
            if defensible(g, unit.pos) {
                obs.on_good_ground += 1.0;
            }
            if !unit.ranged {
                continue;
            }
            obs.ranged_alive += 1.0;
            let reach = theirs
                .iter()
                .map(|foe| g.wdist(unit.pos, foe.pos))
                .min()
                .unwrap_or(i32::MAX);
            // Screened when a friendly stands closer to the nearest enemy than
            // this unit does and is beside it — a body between the shooter and
            // what is coming for it.
            let screened = mine.iter().any(|friend| {
                !std::ptr::eq(*friend, *unit)
                    && g.wdist(friend.pos, unit.pos) <= 1
                    && theirs
                        .iter()
                        .map(|foe| g.wdist(friend.pos, foe.pos))
                        .min()
                        .unwrap_or(i32::MAX)
                        < reach
            });
            if screened {
                obs.screened_ranged += 1.0;
            }
        }

        // Contact statistics only mean something on turns there was contact.
        let contact: Vec<Pos> = theirs
            .iter()
            .filter(|foe| mine.iter().any(|unit| g.wdist(unit.pos, foe.pos) <= 2))
            .map(|foe| foe.pos)
            .collect();
        if contact.is_empty() {
            continue;
        }
        obs.contact_turns += 1;
        let near = |units: &[&Seen]| {
            units
                .iter()
                .filter(|unit| contact.iter().any(|spot| g.wdist(unit.pos, *spot) <= 2))
                .count() as f64
        };
        obs.local_ratio += near(&mine) - near(&theirs);
        obs.enveloped += theirs
            .iter()
            .filter(|foe| {
                mine.iter()
                    .filter(|unit| g.wdist(unit.pos, foe.pos) <= 1)
                    .count()
                    >= 2
            })
            .count() as f64;
    }
}

/// Whether a tile is worth standing on when something is coming: high ground
/// or cover. Deliberately the two facts a general on this list would have
/// recognised, not the engine's full modifier stack.
fn defensible(g: &Game, pos: Pos) -> bool {
    g.map.get(pos).is_some_and(|tile| {
        tile.hills
            || tile
                .feature
                .as_ref()
                .is_some_and(|feature| matches!(feature.as_str(), "forest" | "jungle"))
    })
}

fn split(
    ledgers: &mut (DoctrineLedger, DoctrineLedger),
    side: usize,
) -> (&mut DoctrineLedger, &mut DoctrineLedger) {
    if side == 0 {
        (&mut ledgers.0, &mut ledgers.1)
    } else {
        (&mut ledgers.1, &mut ledgers.0)
    }
}

/// Build the position: an arena of the right size, every tile painted, and
/// both forces mustered with a seeded jitter.
///
/// `None` when a force could not be seated, which the caller reports rather
/// than measuring a half-placed army.
pub fn build(spec: &Position, seed: u64) -> Option<Game> {
    // The Battlefield script is what makes this an arena rather than a world:
    // permanently at war, no upkeep, and a side alive exactly as long as it
    // has a unit standing. Its generated terrain is discarded — every tile
    // below is written.
    //
    // The arena's economy is set to nothing at all. No city, so there is no
    // objective but the enemy army and nowhere to heal; no production and no
    // gold, so no unit arrives that the position did not deploy; no research,
    // so both sides fight the engagement in the era it was written for. The
    // force is the experiment, and an economy is a second variable.
    let mut g = Game::new_with(crate::game::GameOptions {
        map_script: MapScript::Battlefield,
        speed: GameSpeed::Standard.id().to_string(),
        barbarians: false,
        tactics: crate::setup::TacticsRules {
            cities: 0,
            production: 0,
            gold: 0,
            turns_per_tech: 0,
            best_of: 1,
            unique_units: false,
        },
        ..crate::game::GameOptions::new(2, spec.width, spec.height, seed, spec.turns + 8, 0)
    });

    let seeded: Vec<u32> = (0..2).flat_map(|pid| g.player_unit_ids(pid)).collect();
    for uid in seeded {
        g.remove_unit(uid);
    }

    let positions: Vec<Pos> = g.map.tiles.keys().copied().collect();
    for pos in positions {
        if let Some(tile) = g.map.tiles.get_mut(&pos) {
            tile.terrain = "grassland".into();
            tile.feature = None;
            tile.hills = false;
            tile.resource = None;
            tile.improvement = None;
        }
    }
    for (brush, cells) in spec.terrain {
        for (col, row) in *cells {
            let pos = hex::offset_to_axial(*col, *row);
            let Some(tile) = g.map.tiles.get_mut(&pos) else {
                continue;
            };
            match brush {
                Brush::Mountain => tile.terrain = "mountain".into(),
                Brush::Hills => tile.hills = true,
                Brush::Forest => tile.feature = Some("forest".into()),
                Brush::Water => tile.terrain = "coast".into(),
                Brush::Marsh => tile.feature = Some("marsh".into()),
            }
        }
    }

    // The muster: each unit takes the nearest usable tile to where the
    // position puts it, with a seeded nudge so repeated seeds are independent
    // samples of the same shape rather than one game played over and over.
    // A fixed odd constant so the muster is a function of the seed alone and
    // never collides with whatever else that seed drives.
    let mut rng = Rng::new(seed ^ 0xd0c7_5171_ae03_1f47);
    for (role, force) in spec.forces.iter().enumerate() {
        for deploy in *force {
            let wanted = hex::offset_to_axial(deploy.col, deploy.row);
            let spot = muster(&g, wanted, &mut rng)?;
            g.spawn_unit(deploy.kind, role, spot);
        }
    }
    g.record_contact(0, 1);
    Some(g)
}

/// The tile a unit actually forms up on: the nudged one when it is usable,
/// otherwise the nearest usable tile outward from where it was wanted.
fn muster(g: &Game, wanted: Pos, rng: &mut Rng) -> Option<Pos> {
    let mut candidates = vec![wanted];
    // One tile of slop, in a random direction, ahead of the exact spot half
    // the time. Enough to make seeds independent, far too little to turn a
    // deployment into a different one.
    if rng.chance(0.5) {
        let ring = g.wdisk(wanted, 1);
        if !ring.is_empty() {
            candidates.insert(0, ring[rng.below(ring.len())]);
        }
    }
    for radius in 0..=3 {
        candidates.extend(g.wdisk(wanted, radius));
    }
    candidates.into_iter().find(|pos| usable(g, *pos))
}

fn usable(g: &Game, pos: Pos) -> bool {
    g.map.get(pos).is_some_and(|tile| {
        !g.rules.is_water(tile) && g.rules.is_passable(tile) && g.units_at(pos).is_empty()
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::elo::builtin_ai;

    /// Every position has to be buildable and has to seat every unit it asks
    /// for, or its numbers are measuring a force that is not the one written
    /// down. Cheap, and it is the failure that would be easiest to miss.
    #[test]
    fn every_position_seats_the_force_it_specifies() {
        for spec in POSITIONS {
            for seed in [1u64, 2, 3] {
                let game = build(spec, seed)
                    .unwrap_or_else(|| panic!("{} could not be built on seed {seed}", spec.id));
                for role in 0..2 {
                    assert_eq!(
                        game.player_unit_ids(role).len(),
                        spec.forces[role].len(),
                        "{} seated the wrong number of units for role {role}",
                        spec.id
                    );
                }
            }
        }
    }

    /// The terrain is the position. A generated map would make these boards
    /// samples rather than facts, so assert the painted features survive
    /// construction — a defile with no wall is an open field.
    #[test]
    fn the_board_is_painted_not_generated() {
        for spec in POSITIONS {
            let game = build(spec, 7).expect("buildable");
            for (brush, cells) in spec.terrain {
                for (col, row) in *cells {
                    let pos = hex::offset_to_axial(*col, *row);
                    let tile = game
                        .map
                        .get(pos)
                        .unwrap_or_else(|| panic!("{} has no tile at {col},{row}", spec.id));
                    let painted = match brush {
                        Brush::Mountain => tile.terrain.as_str() == "mountain",
                        Brush::Hills => tile.hills,
                        Brush::Forest => tile.feature.as_deref() == Some("forest"),
                        Brush::Water => game.rules.is_water(tile),
                        Brush::Marsh => tile.feature.as_deref() == Some("marsh"),
                    };
                    assert!(painted, "{} lost its {brush:?} at {col},{row}", spec.id);
                }
            }
        }
    }

    /// An arena is at war from the first turn and founds no city, which is
    /// what makes the whole engagement the measurement. If this ever stops
    /// being true the ledger silently starts including an empire.
    #[test]
    fn a_position_is_an_arena_at_war_with_no_cities() {
        let game = build(&POSITIONS[0], 11).expect("buildable");
        assert!(game.is_arena());
        assert!(game.is_at_war(0, 1));
        assert!(game.cities.is_empty(), "an arena founds no city");
    }

    /// The instrument has to move before it can measure. A position that ends
    /// with both forces untouched would report a null for every treatment, and
    /// it would be the harness saying it rather than the agents.
    #[test]
    fn every_position_actually_produces_a_fight() {
        for spec in POSITIONS {
            let result = matched_position(spec, 21, "advanced", "advanced", &builtin_ai);
            assert!(!result.skipped, "{} could not be seated", spec.id);
            let blows = result.a.damage_dealt + result.b.damage_dealt;
            assert!(
                blows > 0.0,
                "{} saw no damage in {} turns of a declared war",
                spec.id,
                result.turns
            );
        }
    }

    /// Damage is conserved: what one side dealt, the other took, and one
    /// side's kills are the other's losses. This is what catches a role swap
    /// that credits the wrong ledger.
    #[test]
    fn the_ledger_is_conserved_between_the_two_sides() {
        for spec in POSITIONS {
            let result = matched_position(spec, 22, "advanced", "advanced", &builtin_ai);
            assert!(!result.skipped);
            assert_eq!(result.a.kills, result.b.losses, "{}", spec.id);
            assert_eq!(result.b.kills, result.a.losses, "{}", spec.id);
            assert!((result.a.damage_dealt - result.b.damage_taken).abs() < 1e-9, "{}", spec.id);
            assert!(
                (result.a.material_destroyed - result.b.material_lost).abs() < 1e-9,
                "{}",
                spec.id
            );
        }
    }

    /// The control. One agent in both roles plays the two role assignments
    /// identically, so the pair must net to exactly zero on every position and
    /// every seed. Nothing read out of this harness means anything until this
    /// holds — it is what says the reported number is the agent and not the
    /// position's own asymmetry.
    #[test]
    fn a_self_match_nets_to_zero_on_every_position() {
        for spec in POSITIONS {
            for seed in [31u64, 32] {
                let result = matched_position(spec, seed, "advanced", "advanced", &builtin_ai);
                if result.skipped {
                    continue;
                }
                assert!(
                    result.paired_difference().abs() < 1e-9,
                    "{} on seed {seed} gave a self-match a paired difference of {}",
                    spec.id,
                    result.paired_difference()
                );
            }
        }
    }

    /// A profile reports `None` rather than a zero when there was nothing to
    /// measure from, and its shares stay inside [0, 1]. A harness that invents
    /// a zero for an engagement that never happened is worse than one that
    /// says nothing.
    #[test]
    fn the_profile_reports_nothing_rather_than_a_zero() {
        let empty = DoctrineLedger::default().profile();
        assert_eq!(empty, DoctrineProfile::default());
        assert!(empty.concentration.is_none());
        assert!(empty.focus.is_none());

        let result = matched_position(&POSITIONS[0], 41, "advanced", "advanced", &builtin_ai);
        let profile = result.a.profile();
        for share in [profile.focus, profile.ground, profile.screen, profile.contact] {
            if let Some(value) = share {
                assert!((0.0..=1.0).contains(&value), "share out of range: {value}");
            }
        }
    }

    /// Identifiers are what a command line and a report key on, so they have
    /// to be unique and findable.
    #[test]
    fn every_position_has_a_unique_identifier() {
        let mut seen = std::collections::BTreeSet::new();
        for spec in POSITIONS {
            assert!(seen.insert(spec.id), "duplicate position id {}", spec.id);
            assert_eq!(position(spec.id).map(|found| found.id), Some(spec.id));
        }
        assert!(position("no_such_position").is_none());
    }
}
