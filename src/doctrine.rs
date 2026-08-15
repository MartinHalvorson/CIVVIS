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
    Position {
        id: "lake_trasimene",
        name: "The march column ambushed",
        provenance: "Hannibal at Lake Trasimene, 217 BC",
        problem: "An army strung out along a shore between water and hills, \
                  with the enemy massed on the high ground above it. A column \
                  is not a formation; it has to become one before it is \
                  destroyed a section at a time.",
        roles: [
            "the column: six units strung along the shore, in march order",
            "the ambush: five on the heights, above the length of it",
        ],
        width: 28,
        height: 12,
        turns: 24,
        terrain: &[
            // The lake, closing the southern flank for the whole length of the
            // road, so the column has nowhere to deploy away from the hills.
            (
                Brush::Water,
                &[
                    (6, 10),
                    (7, 10),
                    (8, 10),
                    (9, 10),
                    (10, 10),
                    (11, 10),
                    (12, 10),
                    (13, 10),
                    (14, 10),
                    (15, 10),
                    (16, 10),
                    (17, 10),
                    (18, 10),
                    (19, 10),
                    (7, 11),
                    (8, 11),
                    (9, 11),
                    (10, 11),
                    (11, 11),
                    (12, 11),
                    (13, 11),
                    (14, 11),
                    (15, 11),
                    (16, 11),
                    (17, 11),
                    (18, 11),
                ],
            ),
            // The heights the ambush comes off, with woods to hold it out of
            // the open until it moves.
            (
                Brush::Hills,
                &[
                    (8, 4),
                    (10, 4),
                    (12, 4),
                    (14, 4),
                    (16, 4),
                    (18, 4),
                    (9, 3),
                    (13, 3),
                    (17, 3),
                ],
            ),
            (Brush::Forest, &[(11, 3), (15, 3), (19, 3), (7, 3)]),
        ],
        forces: [
            &[
                at("warrior", 6, 8),
                at("archer", 8, 8),
                at("spearman", 10, 8),
                at("archer", 12, 8),
                at("swordsman", 14, 8),
                at("warrior", 16, 8),
            ],
            &[
                at("swordsman", 9, 4),
                at("swordsman", 13, 4),
                at("spearman", 11, 4),
                at("horseman", 17, 4),
                at("archer", 15, 4),
            ],
        ],
    },
    Position {
        id: "the_breakthrough",
        name: "The point of rupture",
        provenance: "Bonaparte's masse de rupture; Guderian at Sedan, 1940",
        problem: "A thin line holding everywhere against a fist massed on one \
                  point. Weight at the point of rupture decides it, and a \
                  defence that spreads to cover everything covers nothing.",
        roles: [
            "the fist: six units on one narrow front",
            "the line: six units holding the whole width",
        ],
        width: 26,
        height: 18,
        turns: 24,
        terrain: &[
            // Two woods narrowing the front to three usable approaches, so a
            // line has to choose what it holds rather than standing everywhere.
            (
                Brush::Forest,
                &[
                    (13, 2),
                    (13, 3),
                    (13, 4),
                    (13, 8),
                    (13, 9),
                    (13, 10),
                    (13, 14),
                    (13, 15),
                ],
            ),
            (Brush::Hills, &[(16, 6), (16, 12), (16, 1)]),
        ],
        forces: [
            &[
                at("swordsman", 8, 5),
                at("swordsman", 8, 6),
                at("spearman", 7, 5),
                at("warrior", 7, 6),
                at("archer", 6, 5),
                at("archer", 6, 6),
            ],
            &[
                at("spearman", 18, 1),
                at("warrior", 18, 5),
                at("archer", 19, 6),
                at("spearman", 18, 9),
                at("warrior", 18, 13),
                at("archer", 19, 16),
            ],
        ],
    },
    Position {
        id: "hammer_and_anvil",
        name: "Hammer and anvil",
        provenance: "Alexander at Gaugamela, 331 BC",
        problem: "Infantry fixes the enemy front while something fast goes \
                  round it. The anvil is worth nothing without the hammer, \
                  and the hammer is worth nothing if the anvil breaks first.",
        roles: [
            "the hammer: a holding line and two fast units on a wide flank",
            "the phalanx: heavier infantry, nothing fast, one front",
        ],
        width: 26,
        height: 16,
        turns: 24,
        terrain: &[
            // Broken ground on one flank only, so the wide ride round is open
            // and the short one is not.
            (
                Brush::Forest,
                &[(12, 1), (13, 2), (12, 3), (13, 1), (14, 2)],
            ),
            (Brush::Hills, &[(13, 13), (14, 14), (12, 14)]),
        ],
        forces: [
            &[
                at("spearman", 8, 6),
                at("spearman", 8, 8),
                at("warrior", 7, 7),
                at("archer", 6, 7),
                at("horseman", 7, 12),
                at("horseman", 6, 13),
            ],
            &[
                at("swordsman", 17, 6),
                at("swordsman", 17, 8),
                at("swordsman", 17, 7),
                at("spearman", 18, 6),
                at("spearman", 18, 8),
                at("archer", 19, 7),
            ],
        ],
    },
    Position {
        id: "the_golden_bridge",
        name: "The golden bridge",
        provenance: "Sun Tzu: leave a surrounded enemy a way out",
        problem: "A small force cornered against impassable ground, and a \
                  much larger one that has to finish it. Troops with nowhere \
                  to go fight at a price the arithmetic does not predict.",
        roles: [
            "the cornered: four units with a wall at their back",
            "the encircling: seven units and the whole open field",
        ],
        width: 22,
        height: 14,
        turns: 22,
        terrain: &[
            // A pocket: mountains on three sides with one mouth, so the small
            // force cannot be flanked and cannot leave.
            (
                Brush::Mountain,
                &[
                    (2, 3),
                    (3, 3),
                    (4, 3),
                    (5, 3),
                    (6, 3),
                    (6, 4),
                    (6, 5),
                    (6, 8),
                    (6, 9),
                    (6, 10),
                    (2, 10),
                    (3, 10),
                    (4, 10),
                    (5, 10),
                    (6, 10),
                    (1, 3),
                    (1, 10),
                ],
            ),
            (Brush::Hills, &[(3, 6), (3, 7)]),
        ],
        forces: [
            &[
                at("spearman", 3, 6),
                at("spearman", 3, 7),
                at("archer", 2, 6),
                at("archer", 2, 7),
            ],
            &[
                at("swordsman", 9, 6),
                at("swordsman", 9, 7),
                at("warrior", 10, 5),
                at("warrior", 10, 8),
                at("archer", 11, 6),
                at("archer", 11, 7),
                at("horseman", 12, 6),
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
    /// Units whose arrival step was recorded, and the running sum and sum of
    /// squares of those steps. Kept as sums rather than a computed spread
    /// because a ledger is merged across seedings and runs, and a standard
    /// deviation is not additive.
    arrival_n: f64,
    arrival_sum: f64,
    arrival_sq: f64,
    /// The same three sums over foot units alone, so a spread produced by
    /// cavalry outriding the line can be told apart from one produced by the
    /// line itself coming up piecemeal.
    foot_n: f64,
    foot_sum: f64,
    foot_sq: f64,
    /// The share of the force standing in contact on the turn contact first
    /// occurred, and how many engagements that was measured over. Recorded at
    /// a single instant, upstream of the engagement, which is what makes it
    /// usable as a *cause* rather than a description — see [`DoctrineProfile::vanguard`].
    vanguard_sum: f64,
    vanguard_n: f64,
    /// Engagements whose first-contact instant had not yet seen a casualty,
    /// so the share above is provably untouched by any outcome.
    vanguard_clean: f64,
    /// Units deployed, and units that never reached the enemy at all. The
    /// strongest form of arriving late, and the one form of it that no
    /// difference in movement points can explain.
    deployed: f64,
    absent: f64,
}

impl DoctrineLedger {
    /// Fold another ledger into this one. Public because a report merges a
    /// run's seeds before reading a profile off them: a profile is a ratio,
    /// and averaging ratios across seeds is not the same number as the ratio
    /// of the sums.
    pub fn absorb(&mut self, other: &DoctrineLedger) {
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
        mine.arrival_n += theirs.arrival_n;
        mine.arrival_sum += theirs.arrival_sum;
        mine.arrival_sq += theirs.arrival_sq;
        mine.foot_n += theirs.foot_n;
        mine.foot_sum += theirs.foot_sum;
        mine.foot_sq += theirs.foot_sq;
        mine.deployed += theirs.deployed;
        mine.absent += theirs.absent;
        mine.vanguard_sum += theirs.vanguard_sum;
        mine.vanguard_n += theirs.vanguard_n;
        mine.vanguard_clean += theirs.vanguard_clean;
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
            arrival: spread(obs.arrival_n, obs.arrival_sum, obs.arrival_sq),
            foot_arrival: spread(obs.foot_n, obs.foot_sum, obs.foot_sq),
            absent: (obs.deployed > 0.0).then(|| obs.absent / obs.deployed),
            vanguard: (obs.vanguard_n > 0.0).then(|| obs.vanguard_sum / obs.vanguard_n),
            vanguard_clean: (obs.vanguard_n > 0.0)
                .then(|| obs.vanguard_clean / obs.vanguard_n),
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
    /// Own units near the contact zone less enemy units near it, averaged over
    /// the turns there was contact. Positive is local superiority — the thing
    /// every general on this list was actually trying to arrange. Measured
    /// against a zone shared by both sides, so within one engagement this is
    /// exactly the negative of the opponent's.
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
    /// Spread, in turns, of the moment each unit first came within reach of
    /// an enemy. Low is an army that arrived as a body; high is one that
    /// arrived as a stream and was beaten in instalments. This is the
    /// measurement behind "march divided, fight united" — the one thing every
    /// general in this list agreed on. A unit that never reached the enemy
    /// counts as arriving at the final turn.
    pub arrival: Option<f64>,
    /// The same spread over **foot units alone** — two movement points or
    /// fewer. A horseman with four moves reaches the enemy turns before a
    /// line of spearmen does however well the line is handled, so [`arrival`]
    /// on its own cannot tell an agent that manoeuvres badly from one that
    /// simply uses its cavalry. This column is the one that can: if it moves
    /// with `arrival`, the march is the finding; if it does not, the cavalry
    /// was.
    pub foot_arrival: Option<f64>,
    /// Share of the force that never reached the enemy at all. The strongest
    /// form of arriving late, and the one form of it that no difference in
    /// movement points can explain away.
    pub absent: Option<f64>,
    /// Share of the force standing in contact on the turn contact **first**
    /// occurred. High is an army that met the enemy as a body; low is one
    /// that sent a unit ahead of itself.
    ///
    /// Every other column here is a description of an engagement that has
    /// already happened, and so cannot be used to argue that fighting one way
    /// *causes* winning: an army that is being destroyed stops arriving,
    /// which inflates its own arrival spread. This one is recorded at a
    /// single instant that is upstream of the entire engagement — nothing has
    /// been decided yet — so correlating it against the result is a claim
    /// about cause and not a restatement of the outcome.
    pub vanguard: Option<f64>,
    /// Share of engagements whose first-contact instant had not yet seen a
    /// casualty. Reported so the claim above can be checked rather than
    /// trusted: a side that moves into contact may attack in the same turn,
    /// so the instant is upstream of *almost* every engagement rather than
    /// provably all of them.
    pub vanguard_clean: Option<f64>,
}

/// Population standard deviation from running sums. `None` below two
/// observations, where a spread is not a quantity.
fn spread(n: f64, sum: f64, square_sum: f64) -> Option<f64> {
    (n > 1.0).then(|| {
        let mean = sum / n;
        // Population, not a sample estimate: these are all the units there
        // were, not a draw from a larger force.
        (square_sum / n - mean * mean).max(0.0).sqrt()
    })
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
    /// Foot, in the sense that matters to a march: two movement points or
    /// fewer. A four-move horseman reaches the enemy turns before the line
    /// does no matter how well the line is handled, so the two have to be
    /// separable before an arrival spread can be read as a fact about the
    /// agent rather than about the roster.
    foot: bool,
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
    // "March divided, fight united." The step on which each unit first came
    // within reach of an enemy, so the spread of those steps can say whether
    // the army arrived as a body or as a stream.
    let mut arrival: BTreeMap<u32, u32> = BTreeMap::new();
    // Who was deployed, and whether each is foot. Taken from the opening
    // board rather than from the position's text, so a unit that dies before
    // it ever reaches the enemy is still counted as never having arrived.
    let roster: BTreeMap<u32, (usize, bool)> = previous
        .iter()
        .map(|(uid, unit)| (*uid, (unit.owner, unit.foot)))
        .collect();
    let mut step = 0u32;
    // Recorded once, at the instant contact first exists on the board.
    let mut vanguard_taken = false;

    observe(&previous, &game, &mut ledgers);
    note_arrivals(&previous, &game, step, &mut arrival);
    note_vanguard(&previous, &game, &roster, 0, &mut vanguard_taken, &mut ledgers);
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
        step += 1;
        observe(&previous, &game, &mut ledgers);
        note_arrivals(&previous, &game, step, &mut arrival);
        let fallen = roster.len() - previous.len();
        note_vanguard(&previous, &game, &roster, fallen, &mut vanguard_taken, &mut ledgers);
        // Nothing left to measure once a side has no unit standing.
        if [0usize, 1]
            .iter()
            .any(|side| !previous.values().any(|unit| unit.owner == *side))
        {
            break;
        }
    }
    settle_arrivals(&roster, &arrival, step, &mut ledgers);
    Some((ledgers, game.turn.saturating_sub(start)))
}

/// Record the first step on which each unit stood within two tiles of an
/// enemy. Two tiles rather than one because that is the range at which a unit
/// is part of the engagement rather than walking toward it — the same
/// threshold the contact zone uses.
fn note_arrivals(now: &Snapshot, g: &Game, step: u32, arrival: &mut BTreeMap<u32, u32>) {
    for (uid, unit) in now {
        if arrival.contains_key(uid) {
            continue;
        }
        let engaged = now
            .values()
            .any(|other| other.owner != unit.owner && g.wdist(unit.pos, other.pos) <= 2);
        if engaged {
            arrival.insert(*uid, step);
        }
    }
}

/// Record, once per engagement, the share of each side's force standing in
/// contact at the instant contact first exists.
///
/// The denominator is the force **deployed**, not the force still alive, so a
/// casualty cannot inflate the share of the side that took it. `fallen` is
/// carried in only to mark whether this instant is provably clean of any
/// outcome; a side that moves into contact may attack in the same turn, so
/// the instant is upstream of almost every engagement rather than all of them,
/// and the share of clean ones is reported rather than assumed.
fn note_vanguard(
    now: &Snapshot,
    g: &Game,
    roster: &BTreeMap<u32, (usize, bool)>,
    fallen: usize,
    taken: &mut bool,
    ledgers: &mut (DoctrineLedger, DoctrineLedger),
) {
    if *taken {
        return;
    }
    let in_contact = |unit: &Seen| {
        now.values()
            .any(|other| other.owner != unit.owner && g.wdist(unit.pos, other.pos) <= 2)
    };
    if !now.values().any(in_contact) {
        return;
    }
    *taken = true;
    for side in [0usize, 1] {
        let deployed = roster.values().filter(|(owner, _)| *owner == side).count();
        if deployed == 0 {
            continue;
        }
        let up = now
            .values()
            .filter(|unit| unit.owner == side && in_contact(unit))
            .count();
        let (ledger, _) = split(ledgers, side);
        let obs = &mut ledger.observations;
        obs.vanguard_sum += up as f64 / deployed as f64;
        obs.vanguard_n += 1.0;
        if fallen == 0 {
            obs.vanguard_clean += 1.0;
        }
    }
}

/// Fold the arrival steps into both sides as the running sums a standard
/// deviation needs.
///
/// A unit that never reached the enemy is counted as arriving at the final
/// step. That is deliberate: never arriving is the extreme case of arriving
/// late, and dropping those units would let an army that left half its
/// strength standing in the rear report a *tighter* arrival than one that
/// brought everything up a turn apart.
fn settle_arrivals(
    roster: &BTreeMap<u32, (usize, bool)>,
    arrival: &BTreeMap<u32, u32>,
    last: u32,
    ledgers: &mut (DoctrineLedger, DoctrineLedger),
) {
    for (uid, (side, foot)) in roster {
        // A unit missing from `arrival` never reached the enemy at all.
        let reached = arrival.get(uid).copied();
        let value = f64::from(reached.unwrap_or(last));
        let (ledger, _) = split(ledgers, *side);
        let obs = &mut ledger.observations;
        obs.arrival_n += 1.0;
        obs.arrival_sum += value;
        obs.arrival_sq += value * value;
        obs.deployed += 1.0;
        if reached.is_none() {
            obs.absent += 1.0;
        }
        if *foot {
            obs.foot_n += 1.0;
            obs.foot_sum += value;
            obs.foot_sq += value * value;
        }
    }
}

fn snapshot(g: &Game) -> Snapshot {
    g.units
        .values()
        .filter(|unit| unit.owner < 2 && g.rules.units[unit.kind].class == "military")
        .map(|unit| {
            let spec = g.rules.units.get(&unit.kind);
            let ranged = spec.is_some_and(|spec| spec.range > 0);
            let foot = spec.is_none_or(|spec| spec.moves <= 2.0);
            (
                unit.id,
                Seen {
                    owner: unit.owner,
                    kind: unit.kind.to_string(),
                    hp: unit.hp,
                    pos: unit.pos,
                    ranged,
                    foot,
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
///
/// The contact zone is computed **once for the board** rather than once per
/// side: every unit of either side standing within two tiles of an enemy. Both
/// sides then count against the same set of tiles, which is what makes
/// concentration a real local force ratio — one side's figure is exactly the
/// negative of the other's, and the two rows of a report can be read against
/// each other. Defining it per side instead lets both armies report themselves
/// outnumbered at the same contact, which is not a fact about anything.
fn observe(now: &Snapshot, g: &Game, ledgers: &mut (DoctrineLedger, DoctrineLedger)) {
    let zone: Vec<Pos> = now
        .values()
        .filter(|unit| {
            now.values()
                .any(|other| other.owner != unit.owner && g.wdist(unit.pos, other.pos) <= 2)
        })
        .map(|unit| unit.pos)
        .collect();
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
        if zone.is_empty() {
            continue;
        }
        obs.contact_turns += 1;
        let near = |units: &[&Seen]| {
            units
                .iter()
                .filter(|unit| zone.iter().any(|spot| g.wdist(unit.pos, *spot) <= 2))
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
        // Written out in full rather than spread over a default on purpose: a
        // new arena rule is a new variable in every position here, and it
        // should not arrive silently. Fog stays off — a position is a posed
        // problem, and a commander who cannot see the problem is being
        // measured on reconnaissance instead of on the decision the
        // engagement turned on.
        tactics: crate::setup::TacticsRules {
            cities: 0,
            production: 0,
            gold: 0,
            turns_per_tech: 0,
            best_of: 1,
            unique_units: false,
            fog: false,
            // No flag either: a position is a posed engagement, and a flag
            // would let a walk to a tile end it before the engagement said
            // anything.
            flag: false,
            // The arena's own draw clock, set to the longest it offers so it
            // can never end a position before the ledger's deadline does. A
            // position is read at `spec.turns` and never asks who "won", so
            // the two clocks must not be allowed to disagree.
            turn_limit: *crate::setup::TacticsRules::TURN_LIMITS.last().expect("turn ladder"),
            // And no era choice: a position deploys its exact force by hand,
            // so a rolled or pooled era would re-arm the experiment.
            era: crate::setup::TacticsEra::Start,
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
    let mut candidates = Vec::new();
    // One tile of slop, in a random direction, tried ahead of the exact spot
    // half the time. The ring rather than the disk, so a nudge is a nudge:
    // `wdisk` includes the centre, and drawing from it would leave the unit
    // where it started a seventh of the time it was meant to move.
    if rng.chance(0.5) {
        let ring = g.wring(wanted, 1);
        if !ring.is_empty() {
            candidates.push(ring[rng.below(ring.len())]);
        }
    }
    candidates.push(wanted);
    // Then outward from where it was wanted. `wdisk` is a whole disk and is
    // not ordered by distance, so sorting is what makes "the nearest usable
    // tile" true rather than "some usable tile within three".
    let mut outward = g.wdisk(wanted, 3);
    outward.sort_by_key(|pos| (g.wdist(wanted, *pos), *pos));
    candidates.extend(outward);
    candidates.into_iter().find(|pos| usable(g, *pos))
}

fn usable(g: &Game, pos: Pos) -> bool {
    g.map.get(pos).is_some_and(|tile| {
        !g.rules.is_water(tile) && g.rules.is_passable(tile) && g.units_at(pos).is_empty()
    })
}

/// The paired tests a position result is read through.
///
/// `src/bin/battle_bench.rs` keeps a private copy of these, and this is the
/// one under test — including the overflow that once made the exact sign test
/// report a **confident null on overwhelming evidence**. A binary that grows a
/// third copy should use this instead.
pub mod paired {
    /// Two-sided sign test: the probability of a split at least this lopsided
    /// if the treatment did nothing. Ties are dropped, which is the
    /// conservative convention — counting them as agreement inflates the
    /// harness's own confidence.
    ///
    /// ⚠ The exact form is `tail / 2^n`, and `2f64.powi(n)` is `inf` past
    /// n≈1023. The binomial coefficients overflow with it, `inf / inf` is
    /// `NaN`, and Rust's `NaN.min(1.0)` returns **1.0** — so a 1122-to-317
    /// split once printed `p = 1.0000`. Large n uses the normal approximation
    /// with a continuity correction, which is accurate far beyond anything a
    /// decision here turns on.
    pub fn sign_test(wins: usize, losses: usize) -> f64 {
        let n = wins + losses;
        if n == 0 {
            return 1.0;
        }
        let extreme = wins.max(losses);
        if n > 1000 {
            let mean = n as f64 / 2.0;
            let sd = (n as f64 / 4.0).sqrt();
            let z = ((extreme as f64 - 0.5) - mean) / sd;
            return erfc(z / 2f64.sqrt()).clamp(0.0, 1.0);
        }
        let mut tail = 0.0f64;
        let mut coefficient = 1.0f64;
        for k in 0..=n {
            if k >= extreme || n - k >= extreme {
                tail += coefficient;
            }
            coefficient = coefficient * (n - k) as f64 / (k + 1) as f64;
        }
        (tail / 2f64.powi(n as i32)).clamp(0.0, 1.0)
    }

    /// Mean, standard error, t, and a normal-approximation two-sided p for a
    /// vector of paired differences.
    pub fn paired_t(differences: &[f64]) -> (f64, f64, f64, f64) {
        let n = differences.len();
        if n < 2 {
            return (differences.first().copied().unwrap_or(0.0), 0.0, 0.0, 1.0);
        }
        let mean = differences.iter().sum::<f64>() / n as f64;
        let variance =
            differences.iter().map(|d| (d - mean).powi(2)).sum::<f64>() / (n - 1) as f64;
        let stderr = (variance / n as f64).sqrt();
        if stderr <= 0.0 {
            // No spread at all. A zero mean is the control's exact null and
            // its t is 0, not the infinity that dividing by zero suggests; a
            // non-zero constant difference is unbounded evidence.
            let t = if mean == 0.0 { 0.0 } else { f64::INFINITY };
            return (mean, 0.0, t, if mean == 0.0 { 1.0 } else { 0.0 });
        }
        let t = mean / stderr;
        (mean, stderr, t, erfc(t.abs() / 2f64.sqrt()).clamp(0.0, 1.0))
    }

    /// Pearson correlation between two paired series, with a two-sided p.
    ///
    /// Returns `None` below three pairs, or when either series has no spread
    /// at all — a correlation with a constant is not zero, it is undefined,
    /// and reporting 0.00 there would be the harness asserting independence
    /// it never measured.
    pub fn correlation(xs: &[f64], ys: &[f64]) -> Option<(f64, usize, f64)> {
        let n = xs.len().min(ys.len());
        if n < 3 {
            return None;
        }
        let mean = |values: &[f64]| values[..n].iter().sum::<f64>() / n as f64;
        let (mx, my) = (mean(xs), mean(ys));
        let mut sxy = 0.0;
        let mut sxx = 0.0;
        let mut syy = 0.0;
        for index in 0..n {
            let (dx, dy) = (xs[index] - mx, ys[index] - my);
            sxy += dx * dy;
            sxx += dx * dx;
            syy += dy * dy;
        }
        if sxx <= 0.0 || syy <= 0.0 {
            return None;
        }
        let r = (sxy / (sxx * syy).sqrt()).clamp(-1.0, 1.0);
        // t = r sqrt((n-2)/(1-r^2)), two-sided. A perfect correlation has an
        // infinite t and a p of zero, which is the honest answer for it.
        let p = if r.abs() >= 1.0 {
            0.0
        } else {
            let t = r * ((n - 2) as f64 / (1.0 - r * r)).sqrt();
            erfc(t.abs() / 2f64.sqrt()).clamp(0.0, 1.0)
        };
        Some((r, n, p))
    }

    /// Abramowitz & Stegun 7.1.26, good to ~1.5e-7.
    pub fn erfc(x: f64) -> f64 {
        let z = x.abs();
        let t = 1.0 / (1.0 + 0.5 * z);
        let ans = t
            * (-z * z - 1.265_512_23
                + t * (1.000_023_68
                    + t * (0.374_091_96
                        + t * (0.096_784_18
                            + t * (-0.186_288_06
                                + t * (0.278_868_07
                                    + t * (-1.135_203_98
                                        + t * (1.488_515_87
                                            + t * (-0.822_152_23 + t * 0.170_872_77)))))))))
                .exp();
        if x >= 0.0 {
            ans
        } else {
            2.0 - ans
        }
    }
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

    /// The arena carries its own draw clock, and a position carries a
    /// deadline. If the arena's fires first, the ledger is read on a game the
    /// engine already stopped and every profile silently shortens. Assert the
    /// two clocks cannot disagree.
    #[test]
    fn the_arenas_draw_clock_cannot_end_a_position_early() {
        for spec in POSITIONS {
            let game = build(spec, 61).expect("buildable");
            assert!(
                game.tactics.turn_limit > spec.turns,
                "{} runs {} turns under an arena clock of {}",
                spec.id,
                spec.turns,
                game.tactics.turn_limit
            );
        }
    }

    /// Arrival is the measurement behind "march divided, fight united", so it
    /// has to be a real spread: zero when a force starts in contact together,
    /// and larger for a position that deliberately holds half its army back.
    /// It also must not silently drop units that never arrived — an army that
    /// left half its strength in the rear would otherwise report the tightest
    /// arrival on the board.
    #[test]
    fn arrival_measures_the_spread_of_a_force_reaching_the_enemy() {
        let close = matched_position(
            position("the_golden_bridge").expect("known"),
            71,
            "advanced",
            "advanced",
            &builtin_ai,
        );
        let staggered = matched_position(
            position("the_reserve").expect("known"),
            71,
            "advanced",
            "advanced",
            &builtin_ai,
        );
        let (Some(pocket), Some(reserve)) = (
            close.a_by_role[0].profile().arrival,
            staggered.a_by_role[1].profile().arrival,
        ) else {
            panic!("both positions must produce an arrival spread");
        };
        assert!(pocket >= 0.0 && reserve >= 0.0);
        assert!(
            reserve > pocket,
            "the far reserve ({reserve}) should arrive less together than a \
             force cornered in a pocket ({pocket})"
        );
    }

    /// The whole point of the foot split is that it can disagree with the
    /// all-units figure — that is what lets a spread caused by cavalry
    /// outriding the line be told apart from one caused by the line. Assert
    /// the two are actually different measurements on a position that has
    /// cavalry, and that the foot figure is drawn from fewer units.
    #[test]
    fn the_foot_split_is_a_different_measurement_from_the_whole_force() {
        // hammer_and_anvil is the position built around fast units: role 0
        // holds a line and sends two horsemen wide.
        let spec = position("hammer_and_anvil").expect("known");
        let result = matched_position(spec, 81, "advanced", "advanced", &builtin_ai);
        let profile = result.a_by_role[0].profile();
        let (Some(all), Some(foot)) = (profile.arrival, profile.foot_arrival) else {
            panic!("a position with both foot and horse must report both spreads");
        };
        assert!(all >= 0.0 && foot >= 0.0);
        assert!(
            (all - foot).abs() > 1e-9,
            "the foot split reproduced the whole-force figure exactly ({all}), \
             so it is not separating anything"
        );
        // And on a position with no cavalry at all the two must coincide,
        // because then every unit is foot.
        let infantry = position("the_golden_bridge").expect("known");
        let result = matched_position(infantry, 81, "advanced", "advanced", &builtin_ai);
        let profile = result.a_by_role[0].profile();
        assert_eq!(
            profile.arrival, profile.foot_arrival,
            "a force of nothing but foot must report the same spread twice"
        );
    }

    /// `absent` is the one form of arriving late that no difference in
    /// movement points can explain away, so it has to be a real share of a
    /// real denominator — every deployed unit, including any that died before
    /// they ever reached the enemy.
    #[test]
    fn absence_is_a_share_of_the_whole_force() {
        for spec in POSITIONS {
            let result = matched_position(spec, 82, "advanced", "advanced", &builtin_ai);
            if result.skipped {
                continue;
            }
            for role in 0..2 {
                let Some(absent) = result.a_by_role[role].profile().absent else {
                    panic!("{} role {role} reported no absence share", spec.id);
                };
                assert!(
                    (0.0..=1.0).contains(&absent),
                    "{} role {role} reported an absence share of {absent}",
                    spec.id
                );
            }
        }
    }

    /// A correlation with a constant series is undefined, not zero. Reporting
    /// 0.00 there would be the harness asserting an independence it never
    /// measured — which is the same class of error as the sign test's
    /// confident null.
    #[test]
    fn a_correlation_with_no_spread_is_nothing_rather_than_zero() {
        assert!(paired::correlation(&[1.0, 2.0], &[1.0, 2.0]).is_none(), "too few pairs");
        assert!(
            paired::correlation(&[1.0, 1.0, 1.0], &[1.0, 2.0, 3.0]).is_none(),
            "a constant x has no correlation, not a zero one"
        );
        let (r, n, p) = paired::correlation(&[1.0, 2.0, 3.0, 4.0], &[2.0, 4.0, 6.0, 8.0])
            .expect("a perfect line correlates");
        assert!((r - 1.0).abs() < 1e-12, "r = {r}");
        assert_eq!(n, 4);
        assert_eq!(p, 0.0);
        let (r, _, _) = paired::correlation(&[1.0, 2.0, 3.0, 4.0], &[8.0, 6.0, 4.0, 2.0])
            .expect("a perfect inverse correlates");
        assert!((r + 1.0).abs() < 1e-12, "r = {r}");
    }

    /// The vanguard is the one column meant to support a causal claim, so its
    /// two guarantees have to hold: it is a share of the force deployed, and
    /// the instant it is taken at is upstream of essentially every engagement.
    /// If that second figure were low the column would be worthless for the
    /// job it exists to do.
    #[test]
    fn the_vanguard_is_recorded_before_the_engagement_decides_anything() {
        let mut clean = 0.0;
        let mut total = 0.0;
        for spec in POSITIONS {
            let result = matched_position(spec, 91, "advanced", "basic", &builtin_ai);
            if result.skipped {
                continue;
            }
            for ledger in [&result.a, &result.b] {
                let profile = ledger.profile();
                let (Some(vanguard), Some(share)) = (profile.vanguard, profile.vanguard_clean)
                else {
                    panic!("{} produced no vanguard", spec.id);
                };
                assert!(
                    (0.0..=1.0).contains(&vanguard),
                    "{} reported a vanguard of {vanguard}",
                    spec.id
                );
                clean += share;
                total += 1.0;
            }
        }
        let overall = clean / total;
        assert!(
            overall > 0.9,
            "only {:.0}% of first-contact instants were clean of a casualty, so \
             the vanguard cannot carry a causal claim",
            overall * 100.0
        );
    }

    /// The muster has to actually vary, or every seed replays one game and a
    /// run of 60 has the resolving power of a run of one. It also has to stay
    /// close to the written deployment, or the position stops being the
    /// position.
    #[test]
    fn the_muster_varies_between_seeds_without_leaving_the_position() {
        let spec = &POSITIONS[0];
        let mut layouts = std::collections::BTreeSet::new();
        for seed in 0u64..12 {
            let game = build(spec, seed).expect("buildable");
            let mut layout: Vec<(usize, Pos)> = Vec::new();
            for role in 0..2 {
                for uid in game.player_unit_ids(role) {
                    layout.push((role, game.units[&uid].pos));
                }
            }
            layout.sort();
            // Every unit is within the slop the muster is allowed, measured
            // against the nearest cell its own force asked for.
            for (role, pos) in &layout {
                let nearest = spec.forces[*role]
                    .iter()
                    .map(|deploy| game.wdist(hex::offset_to_axial(deploy.col, deploy.row), *pos))
                    .min()
                    .unwrap_or(i32::MAX);
                assert!(
                    nearest <= 3,
                    "{} seated a unit {nearest} tiles from anything it deployed",
                    spec.id
                );
            }
            layouts.insert(layout);
        }
        assert!(
            layouts.len() > 1,
            "12 seeds produced one layout: the muster is not varying and every \
             seed is the same game"
        );
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

    /// Concentration is a local force ratio, so within one engagement one
    /// side's figure must be exactly the negative of the other's. Measured
    /// against a per-side contact set instead of a shared zone, both armies
    /// can report themselves outnumbered at the same contact — which is not a
    /// fact about anything, and is invisible until someone tries to read the
    /// two rows against each other.
    #[test]
    fn concentration_is_a_local_force_ratio_and_sums_to_zero() {
        for spec in POSITIONS {
            let result = matched_position(spec, 51, "advanced", "advanced", &builtin_ai);
            if result.skipped {
                continue;
            }
            for role in 0..2 {
                // Within one seating the two roles share a board, so their
                // summed local ratios must cancel turn by turn.
                let (Some(first), Some(second)) = (
                    result.a_by_role[role].profile().concentration,
                    result.b_by_role[1 - role].profile().concentration,
                ) else {
                    continue;
                };
                assert!(
                    (first + second).abs() < 1e-9,
                    "{} role {role}: {first} against {second} does not cancel",
                    spec.id
                );
            }
        }
    }

    /// The sign test must not report a confident null on overwhelming
    /// evidence. The exact form overflows past n≈1023 and `NaN.min(1.0)` in
    /// Rust returns 1.0, which is how a 1122-to-317 split once printed
    /// `p = 1.0000`. Assert the large-n branch on exactly that split, and
    /// assert the two branches agree either side of the switch.
    #[test]
    fn the_sign_test_does_not_overflow_into_a_confident_null() {
        let p = paired::sign_test(1_122, 317);
        assert!(p.is_finite() && p < 1e-6, "overwhelming split reported p = {p}");
        assert!((paired::sign_test(0, 0) - 1.0).abs() < 1e-12);
        assert!((paired::sign_test(5, 5) - 1.0).abs() < 1e-9, "an even split is p = 1");
        // Either side of the exact/approximate switch, on the same shape.
        let exact = paired::sign_test(600, 400);
        let approximate = paired::sign_test(601, 400);
        assert!(
            (exact - approximate).abs() < 5e-3,
            "the branches disagree: {exact} against {approximate}"
        );
    }

    /// A paired t on constant differences has no spread, and reporting a
    /// finite p there would be the harness inventing precision. One difference
    /// is a number, not a result.
    #[test]
    fn the_paired_t_handles_no_spread_and_too_few_pairs() {
        let (mean, stderr, _, p) = paired::paired_t(&[4.0, 4.0, 4.0]);
        assert!((mean - 4.0).abs() < 1e-12);
        assert_eq!(stderr, 0.0);
        assert_eq!(p, 0.0, "a constant non-zero difference is not a null");
        let (mean, _, _, p) = paired::paired_t(&[7.0]);
        assert!((mean - 7.0).abs() < 1e-12);
        assert_eq!(p, 1.0, "one pair licenses nothing");
        let (_, _, _, p) = paired::paired_t(&[0.0, 0.0, 0.0]);
        assert_eq!(p, 1.0);
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
