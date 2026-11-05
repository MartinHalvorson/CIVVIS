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
//! zero** on every seed, and `a_self_match_nets_to_zero_on_every_position`
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

use serde::{Deserialize, Serialize};

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
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
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

/// A city on a position's board: whose it is, where it stands, and what
/// shape its defences are in.
///
/// The arena has always been able to hold a city — the stock Tactics battle
/// gives each side one — but a *position* could not place it, so the arena
/// could pose every tactical problem except the one the live seat keeps
/// losing: eleven declared wars, four sieges to 180-190 of 200, and no
/// capture. An assault is a different problem from a field fight and needs
/// its own board.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Stronghold {
    /// Which side holds it, in role order.
    pub role: usize,
    pub col: i32,
    pub row: i32,
    /// The city's own hit points. A city at 200 is untouched; the live
    /// seat's four abandoned sieges each left one at 180-190.
    pub hp: i32,
    /// The outer-defence pool walls give it, which a melee unit cannot
    /// capture through. Zero is an unwalled city and the only kind a
    /// controller has ever taken here.
    pub wall_hp: i32,
}

/// Shorthand so a board's cities read as a list of placements.
const fn city(role: usize, col: i32, row: i32, hp: i32, wall_hp: i32) -> Stronghold {
    Stronghold {
        role,
        col,
        row,
        hp,
        wall_hp,
    }
}

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
    /// Cities on the board. Empty for a field position, which is every one
    /// of them written before 2026-08-27.
    pub cities: &'static [Stronghold],
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

/// One unit of a board the arena can play: what it is, where it stands, and
/// the state it carried when the board was taken.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Placed {
    pub kind: String,
    pub col: i32,
    pub row: i32,
    /// Hit points on deployment. A hand-built position deploys whole units;
    /// a board captured from a game deploys the army as it stood.
    #[serde(default = "whole")]
    pub hp: i32,
    #[serde(default)]
    pub promotions: Vec<String>,
}

fn whole() -> i32 {
    100
}

/// A city on an owned board; the serialisable twin of [`Stronghold`].
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Held {
    pub role: usize,
    pub col: i32,
    pub row: i32,
    #[serde(default = "whole")]
    pub hp: i32,
    #[serde(default)]
    pub wall_hp: i32,
    /// The name it had in the game it came from, so a captured siege is
    /// recognisable in a report.
    #[serde(default)]
    pub name: String,
}

impl From<&Stronghold> for Held {
    fn from(spec: &Stronghold) -> Self {
        Held {
            role: spec.role,
            col: spec.col,
            row: spec.row,
            hp: spec.hp,
            wall_hp: spec.wall_hp,
            name: String::new(),
        }
    }
}

/// A board the arena can play, owned and serialisable: every hand-built
/// [`Position`] converts to one, and [`capture_engagement`] takes one from a
/// real game at the moment two armies first came within reach of each other.
///
/// The hand-built positions are a *curriculum* — eleven decisions worth being
/// able to make. A captured engagement is a *sample* — the fights the
/// controller actually gets into, on the ground it actually gets into them
/// on, with the army it actually brought. A file of them is the distribution
/// the curriculum was never meant to be, and it is what lets a tactical change
/// be priced where its effect is instead of on a whole-game win rate that
/// resolves fourteen kills a game. Written to JSON so a board taken on one
/// machine replays on every other, which is the same guarantee `POSITIONS`
/// gives by being source.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Engagement {
    /// Stable identifier, used on the command line and in reports.
    pub id: String,
    #[serde(default)]
    pub name: String,
    /// Where the board came from: the general, or the seed and turn.
    #[serde(default)]
    pub provenance: String,
    #[serde(default)]
    pub problem: String,
    #[serde(default)]
    pub roles: [String; 2],
    pub width: i32,
    pub height: i32,
    /// Turns of fighting before the ledger is read.
    pub turns: u32,
    #[serde(default)]
    pub terrain: Vec<(Brush, Vec<(i32, i32)>)>,
    /// River edges by cell, in the engine's six-edge order. A hand-built
    /// position carries none — it fakes a river line with coast — but a
    /// captured board keeps the real crossings, because the river penalty is
    /// one of the terms the fighting turns on.
    #[serde(default)]
    pub rivers: Vec<((i32, i32), [bool; 6])>,
    /// Cities on the board, with the hit points and walls they carried.
    /// Empty for a field board.
    #[serde(default)]
    pub cities: Vec<Held>,
    /// Forces in role order.
    pub forces: [Vec<Placed>; 2],
    /// Whether units recover on this board. Off is the arena's own rule and
    /// the curriculum's: one engagement, permanent damage, a trade you win
    /// stays won. On is a campaign: the unit that steps back is whole again
    /// in a few turns, and preserving it is worth something. The two are
    /// different questions, and a board says which it is asking.
    #[serde(default)]
    pub heal: bool,
}

impl From<&Position> for Engagement {
    fn from(spec: &Position) -> Self {
        Engagement {
            id: spec.id.to_string(),
            name: spec.name.to_string(),
            provenance: spec.provenance.to_string(),
            problem: spec.problem.to_string(),
            roles: [spec.roles[0].to_string(), spec.roles[1].to_string()],
            width: spec.width,
            height: spec.height,
            turns: spec.turns,
            terrain: spec
                .terrain
                .iter()
                .map(|(brush, cells)| (*brush, cells.to_vec()))
                .collect(),
            rivers: Vec::new(),
            cities: spec.cities.iter().map(Held::from).collect(),
            forces: [
                spec.forces[0].iter().map(Placed::from).collect(),
                spec.forces[1].iter().map(Placed::from).collect(),
            ],
            heal: false,
        }
    }
}

impl From<&Deploy> for Placed {
    fn from(deploy: &Deploy) -> Self {
        Placed {
            kind: deploy.kind.to_string(),
            col: deploy.col,
            row: deploy.row,
            hp: whole(),
            promotions: Vec::new(),
        }
    }
}

impl Engagement {
    /// Total production cost of one side's force; see [`Position::material`].
    pub fn material(&self, role: usize, rules: &crate::rules::Rules) -> f64 {
        self.forces[role]
            .iter()
            .map(|unit| {
                rules
                    .units
                    .get(unit.kind.as_str())
                    .map_or(0.0, |spec| spec.cost)
            })
            .sum()
    }

    /// Every hand-built position, as boards.
    pub fn curriculum() -> Vec<Engagement> {
        POSITIONS.iter().map(Engagement::from).collect()
    }

    /// Read a file of boards, as [`to_json`] writes it. Refuses a board that
    /// asks for a unit the ruleset does not have or a cell off its own board,
    /// so a typo fails here rather than seating half an army.
    pub fn from_json(text: &str) -> Result<Vec<Engagement>, String> {
        let boards: Vec<Engagement> =
            serde_json::from_str(text).map_err(|error| format!("engagements: {error}"))?;
        let rules = crate::rules::Rules::embedded();
        for board in &boards {
            if board.width < 4 || board.height < 4 || board.turns == 0 {
                return Err(format!("{}: a board needs a size and a clock", board.id));
            }
            for held in &board.cities {
                if held.role > 1 {
                    return Err(format!("{}: a city belongs to role 0 or 1", board.id));
                }
                if held.col < 0
                    || held.col >= board.width
                    || held.row < 0
                    || held.row >= board.height
                {
                    return Err(format!(
                        "{}: a city stands off the board at {},{}",
                        board.id, held.col, held.row
                    ));
                }
            }
            for (role, force) in board.forces.iter().enumerate() {
                for unit in force {
                    if !rules.units.contains_key(unit.kind.as_str()) {
                        return Err(format!("{}: no unit `{}`", board.id, unit.kind));
                    }
                    if unit.col < 0
                        || unit.col >= board.width
                        || unit.row < 0
                        || unit.row >= board.height
                    {
                        return Err(format!(
                            "{}: role {role} places a {} off the board at {},{}",
                            board.id, unit.kind, unit.col, unit.row
                        ));
                    }
                }
            }
        }
        Ok(boards)
    }

    /// Write boards the way [`from_json`] reads them.
    pub fn to_json(boards: &[Engagement]) -> String {
        serde_json::to_string_pretty(boards).expect("engagements serialise")
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
        cities: &[],
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
        cities: &[],
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
        cities: &[],
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
        cities: &[],
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
        cities: &[],
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
        cities: &[],
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
        cities: &[],
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
        cities: &[],
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
        cities: &[],
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
        cities: &[],
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
        cities: &[],
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
    Position {
        id: "the_storming",
        name: "The storming",
        provenance: "Vauban's breach; every siege that ended in an assault rather than a blockade",
        problem: "A walled city with a garrison, and a force with the siege \
                  train to breach it. Walls are an outer pool a melee unit \
                  cannot capture through, so the assault has an order: break \
                  the wall, then take the city, and a body that storms an \
                  unbreached wall is spent for nothing.",
        roles: [
            "the besieger: two catapults, three melee and an archer, on the open side",
            "the garrison: three defenders behind a hundred points of wall",
        ],
        width: 24,
        height: 14,
        turns: 34,
        terrain: &[
            (Brush::Hills, &[(16, 6), (17, 5)]),
            (Brush::Forest, &[(13, 4), (13, 8), (14, 3)]),
        ],
        cities: &[city(1, 17, 6, 200, 100)],
        forces: [
            &[
                at("catapult", 9, 5),
                at("catapult", 9, 7),
                at("swordsman", 11, 5),
                at("swordsman", 11, 7),
                at("warrior", 11, 6),
                at("archer", 10, 6),
            ],
            &[
                at("spearman", 18, 6),
                at("warrior", 17, 7),
                at("archer", 18, 5),
            ],
        ],
    },
    Position {
        id: "the_relief",
        name: "The relief",
        provenance: "Every siege raised from inside: the garrison that held its wall until the column behind it came up",
        problem: "A walled city with a thin garrison and a relief force two \
                  tiles behind it, against an assault that outnumbers the \
                  garrison three to one. The defence has to decide what the \
                  city tile is for, who stands on the ring, and whether the \
                  relief fights at the wall or in front of it — the first \
                  board on which holding a city, rather than taking one, is \
                  the problem posed.",
        roles: [
            "the assault: a catapult, two swordsmen, a warrior, an archer and a horseman on the open side",
            "the defence: two in the city's shadow and a relief of four two tiles behind it",
        ],
        width: 24,
        height: 14,
        turns: 34,
        terrain: &[
            (Brush::Hills, &[(16, 5), (16, 7)]),
            (Brush::Forest, &[(13, 6), (14, 4), (14, 8)]),
        ],
        cities: &[city(1, 17, 6, 200, 100)],
        forces: [
            &[
                at("catapult", 9, 6),
                at("swordsman", 11, 5),
                at("swordsman", 11, 7),
                at("warrior", 11, 6),
                at("archer", 10, 6),
                at("horseman", 12, 6),
            ],
            &[
                at("spearman", 18, 6),
                at("archer", 18, 5),
                at("swordsman", 20, 5),
                at("spearman", 20, 7),
                at("archer", 20, 6),
                at("horseman", 21, 6),
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
    /// Cities taken from the other side, and lost to it. Zero on a field
    /// board, which is every position written before 2026-08-27 — and the
    /// number the live seat has never once put above zero in a real war.
    pub cities_taken: usize,
    pub cities_lost: usize,
    /// Units lost that were already at or below [`SALVAGEABLE_HP`] on the
    /// turn before they died — a loss the controller had a turn's warning
    /// of and could have answered by rotating, withdrawing or healing. The
    /// rest were killed from a health it had no reason to act on.
    ///
    /// The distinction only became worth making when a board could heal
    /// (`--heal`): with nothing to recover to, "should have pulled it out"
    /// was not a real alternative, and `swap-rotation` then measured the
    /// same either way. This is the column that says which kind of loss a
    /// change is actually preventing.
    pub losses_when_salvageable: usize,
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
        self.cities_taken += other.cities_taken;
        self.cities_lost += other.cities_lost;
        self.losses_when_salvageable += other.losses_when_salvageable;
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
        let per_contact =
            |total: f64| (obs.contact_turns > 0).then(|| total / obs.contact_turns as f64);
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
            vanguard_clean: (obs.vanguard_n > 0.0).then(|| obs.vanguard_clean / obs.vanguard_n),
            salvageable: (self.losses > 0)
                .then(|| self.losses_when_salvageable as f64 / self.losses as f64),
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
    /// line of spearmen does however well the line is handled, so [`Self::arrival`]
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
    /// Share of own losses that were already at or below
    /// [`SALVAGEABLE_HP`] the turn before — losses the controller had a
    /// turn's warning of. `None` when nothing was lost, which is a
    /// different statement from none of them being preventable.
    pub salvageable: Option<f64>,
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
    matched_engagement(&Engagement::from(spec), seed, name_a, name_b, agent)
}

/// [`matched_position`] for any board — a converted position or a captured
/// engagement.
pub fn matched_engagement(
    spec: &Engagement,
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
    spec: &Engagement,
    seed: u64,
    seats: [&str; 2],
    agent: &dyn Fn(&str, u64) -> Box<dyn Ai>,
) -> Option<((DoctrineLedger, DoctrineLedger), u32)> {
    let mut game = build_engagement(spec, seed)?;
    let mut ais: Vec<Box<dyn Ai>> = (0..2)
        .map(|pid| agent(seats[pid], seed.wrapping_add(pid as u64)))
        .collect();

    let mut ledgers = (DoctrineLedger::default(), DoctrineLedger::default());
    let mut previous = snapshot(&game);
    let mut held = city_owners(&game);
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
    note_vanguard(
        &previous,
        &game,
        &roster,
        0,
        &mut vanguard_taken,
        &mut ledgers,
    );
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
        let now_held = city_owners(&game);
        account_cities(&held, &now_held, &mut ledgers);
        held = now_held;
        previous = now;
        step += 1;
        observe(&previous, &game, &mut ledgers);
        note_arrivals(&previous, &game, step, &mut arrival);
        let fallen = roster.len() - previous.len();
        note_vanguard(
            &previous,
            &game,
            &roster,
            fallen,
            &mut vanguard_taken,
            &mut ledgers,
        );
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

/// Which side holds each city on the board, so a change of hands can be
/// scored. Keyed on the city, not the tile: a captured city keeps its id.
fn city_owners(g: &Game) -> BTreeMap<u32, usize> {
    g.cities
        .values()
        .filter(|city| city.owner < 2)
        .map(|city| (city.id, city.owner))
        .collect()
}

/// Credit every city that changed hands between two readings to the side
/// that took it, and charge it to the side that lost it.
fn account_cities(
    before: &BTreeMap<u32, usize>,
    after: &BTreeMap<u32, usize>,
    ledgers: &mut (DoctrineLedger, DoctrineLedger),
) {
    for (cid, was) in before {
        let Some(now) = after.get(cid) else {
            continue;
        };
        if now == was {
            continue;
        }
        let (taker, loser) = if *now == 0 {
            (&mut ledgers.0, &mut ledgers.1)
        } else {
            (&mut ledgers.1, &mut ledgers.0)
        };
        taker.cities_taken += 1;
        loser.cities_lost += 1;
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
                let salvageable = seen.hp <= SALVAGEABLE_HP;
                let (mine, theirs) = split(ledgers, side);
                mine.losses += 1;
                if salvageable {
                    mine.losses_when_salvageable += 1;
                }
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
    build_engagement(&Engagement::from(spec), seed)
}

/// [`build`] for any board. A captured engagement also carries its river
/// crossings, each unit's hit points and promotions, and whether the board
/// heals.
pub fn build_engagement(spec: &Engagement, seed: u64) -> Option<Game> {
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
            turn_limit: *crate::setup::TacticsRules::TURN_LIMITS
                .last()
                .expect("turn ladder"),
            // And no era choice: a position deploys its exact force by hand,
            // so a rolled or pooled era would re-arm the experiment.
            era: crate::setup::TacticsEra::Start,
            // Healing is the board's own question; see `Engagement::heal`.
            heal: spec.heal,
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
    for (brush, cells) in &spec.terrain {
        for (col, row) in cells {
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
    for ((col, row), edges) in &spec.rivers {
        if let Some(tile) = g.map.tiles.get_mut(&hex::offset_to_axial(*col, *row)) {
            tile.river_edges = *edges;
        }
    }

    // The cities, before the army, so the deployment musters around them
    // rather than onto them. `place_city` is the arena's own path — the
    // `FoundCity` *action* is refused on a battlefield, and rightly, because
    // a position states what is held rather than letting a settler decide.
    for held in &spec.cities {
        let pos = hex::offset_to_axial(held.col, held.row);
        if g.map.get(pos).is_none() {
            continue;
        }
        let name = (!held.name.is_empty()).then(|| held.name.clone());
        let cid = g.place_city(held.role.min(1), pos, name);
        if let Some(cityctl) = g.cities.get_mut(&cid) {
            cityctl.hp = held.hp.clamp(1, 200);
            cityctl.wall_hp = held.wall_hp.max(0);
        }
        // Walls are ordinarily derived from the buildings a city has built,
        // and a board's city has built nothing. The same override the live
        // mirror uses for the host's number states the pool directly, so a
        // walled position is walled and an unwalled one cannot silently grow
        // a wall from a ruleset default.
        std::sync::Arc::make_mut(&mut g.observed_city_max_wall_hp).insert(cid, held.wall_hp.max(0));
    }

    // The muster: each unit takes the nearest usable tile to where the
    // position puts it, with a seeded nudge so repeated seeds are independent
    // samples of the same shape rather than one game played over and over.
    // A fixed odd constant so the muster is a function of the seed alone and
    // never collides with whatever else that seed drives.
    let mut rng = Rng::new(seed ^ 0xd0c7_5171_ae03_1f47);
    for (role, force) in spec.forces.iter().enumerate() {
        for deploy in force {
            let wanted = hex::offset_to_axial(deploy.col, deploy.row);
            let spot = muster(&g, wanted, &mut rng)?;
            let uid = g.spawn_unit(&deploy.kind, role, spot);
            // A captured board deploys the army as it stood: the hit points
            // it had and the promotions it had earned. A hand-built position
            // says nothing here and gets whole, unpromoted units, as before.
            let earned: Vec<crate::name::Name> = deploy
                .promotions
                .iter()
                .filter(|name| g.rules.promotions.get(name.as_str()).is_some())
                .map(|name| crate::name::Name::new(name))
                .collect();
            if let Some(unit) = g.units.get_mut(&uid) {
                unit.hp = deploy.hp.clamp(1, 100);
                unit.promotions.extend(earned);
            }
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
    // A city's own tile is not a muster tile: a position states which side
    // holds it, and seating a unit there at setup would answer the
    // assault before the first turn.
    g.city_at(pos).is_none()
        && g.map.get(pos).is_some_and(|tile| {
            !g.rules.is_water(tile) && g.rules.is_passable(tile) && g.units_at(pos).is_empty()
        })
}

/// The range at which a unit is part of an engagement rather than walking
/// toward it — the contact zone's own threshold, and `note_arrivals`'.
pub const CONTACT_RANGE: i32 = 2;

/// A unit at or below this on the turn before it died was one the
/// controller had a turn's warning about. Thirty: below `withdraw_hp` (45,
/// the line recovery already uses), so the column counts units the
/// controller had already been told to pull out and did not, rather than
/// every unit that happened to be scratched.
pub const SALVAGEABLE_HP: i32 = 30;

/// Land military units of one player: what a captured board can seat. Ships
/// and aircraft are left out — the arena's muster refuses water, and the
/// engagement being captured is the one between the armies.
fn land_army(g: &Game, pid: usize) -> Vec<&crate::game::Unit> {
    g.units
        .values()
        .filter(|unit| unit.owner == pid)
        .filter(|unit| {
            let spec = &g.rules.units[unit.kind];
            spec.class == "military" && spec.domain.as_deref().is_none_or(|domain| domain == "land")
        })
        .collect()
}

/// Take the board around the contact between `a`'s army and `b`'s: every
/// tile within `radius` of the units in contact, with its hills, cover,
/// water, mountains and river crossings, and both sides' land units standing
/// inside it as they stood — hit points and promotions included. `None` when
/// the two armies are not within [`CONTACT_RANGE`] of each other, or when
/// either side has fewer than two units in the window: one unit walking into
/// an army is a casualty, not an engagement.
///
/// What a captured board loses, stated so nobody reads it as the whole
/// game: cities (an arena founds none, and a siege is its own instrument),
/// third parties (only the two armies are seated), improvements and
/// resources (an arena has no economy), and anything outside the window.
/// The board is played from role 0's perspective as `a` and role 1's as `b`,
/// both roles by both agents, exactly like a hand-built position.
pub fn capture_engagement(
    g: &Game,
    a: usize,
    b: usize,
    radius: i32,
    turns: u32,
    id: &str,
) -> Option<Engagement> {
    let ours = land_army(g, a);
    let theirs = land_army(g, b);
    let in_contact: Vec<Pos> = ours
        .iter()
        .filter(|unit| {
            theirs
                .iter()
                .any(|foe| g.wdist(unit.pos, foe.pos) <= CONTACT_RANGE)
        })
        .chain(theirs.iter().filter(|foe| {
            ours.iter()
                .any(|unit| g.wdist(unit.pos, foe.pos) <= CONTACT_RANGE)
        }))
        .map(|unit| unit.pos)
        .collect();
    if in_contact.is_empty() {
        return None;
    }
    // The medoid of the contact: the unit position nearest every other, so
    // the window is centred on the fight and not on an outlier.
    let centre = *in_contact.iter().min_by_key(|pos| {
        (
            in_contact
                .iter()
                .map(|other| g.wdist(**pos, *other))
                .sum::<i32>(),
            **pos,
        )
    })?;
    let (centre_col, centre_row) = hex::axial_to_offset(centre.0, centre.1);
    // The board is offset (column, row) with odd rows shifted, so a window
    // keeps its geometry only if every row moves by an EVEN count — a shift
    // of seven rows would put the units of an even row a half-hex off the
    // units of an odd one. The centre therefore lands at `radius + 1` or one
    // row further, whichever makes the shift even; columns shift freely. A
    // cylinder's seam is unwrapped around the centre so a window across it
    // stays whole.
    let mut origin_row = centre_row - radius - 1;
    if origin_row.rem_euclid(2) != 0 {
        origin_row -= 1;
    }
    let width = 2 * radius + 3;
    let height = centre_row - origin_row + radius + 2;
    let seam = g
        .map
        .wraps_east_west()
        .then_some(g.map.width)
        .filter(|width| *width > 0);
    let cell = |pos: Pos| -> Option<(i32, i32)> {
        let (col, row) = hex::axial_to_offset(pos.0, pos.1);
        let mut dcol = col - centre_col;
        if let Some(width) = seam {
            dcol = (dcol + width / 2).rem_euclid(width) - width / 2;
        }
        let out = (dcol + radius + 1, row - origin_row);
        (out.0 >= 0 && out.0 < width && out.1 >= 0 && out.1 < height).then_some(out)
    };
    let window = g.wdisk(centre, radius);
    let mut strokes: BTreeMap<u8, Vec<(i32, i32)>> = BTreeMap::new();
    let mut rivers = Vec::new();
    for pos in &window {
        let (Some(tile), Some(at)) = (g.map.get(*pos), cell(*pos)) else {
            continue;
        };
        let mut brushes: Vec<Brush> = Vec::new();
        if tile.terrain.as_str() == "mountain" {
            brushes.push(Brush::Mountain);
        } else if g.rules.is_water(tile) {
            brushes.push(Brush::Water);
        } else {
            if tile.hills {
                brushes.push(Brush::Hills);
            }
            match tile.feature.as_deref() {
                Some("forest") | Some("jungle") => brushes.push(Brush::Forest),
                Some("marsh") => brushes.push(Brush::Marsh),
                _ => {}
            }
        }
        for brush in brushes {
            strokes.entry(brush as u8).or_default().push(at);
        }
        if tile.river_edges.iter().any(|edge| *edge) {
            rivers.push((at, tile.river_edges));
        }
    }
    let order = [
        Brush::Mountain,
        Brush::Hills,
        Brush::Forest,
        Brush::Water,
        Brush::Marsh,
    ];
    let terrain: Vec<(Brush, Vec<(i32, i32)>)> = order
        .iter()
        .filter_map(|brush| strokes.remove(&(*brush as u8)).map(|cells| (*brush, cells)))
        .collect();
    let seat = |army: &[&crate::game::Unit]| -> Vec<Placed> {
        army.iter()
            .filter(|unit| g.wdist(unit.pos, centre) <= radius)
            .filter_map(|unit| {
                let (col, row) = cell(unit.pos)?;
                Some(Placed {
                    kind: unit.kind.to_string(),
                    col,
                    row,
                    hp: unit.hp,
                    promotions: unit
                        .promotions
                        .iter()
                        .map(|name| name.to_string())
                        .collect(),
                })
            })
            .collect()
    };
    let forces = [seat(&ours), seat(&theirs)];
    if forces.iter().any(|force| force.len() < 2) {
        return None;
    }
    // ⭐ AND THE CITY, IF THE FIGHT IS OVER ONE. A siege captured as a field
    // fight is not the fight: the whole question the live seat keeps failing
    // — four sieges to 180-190 of 200 and no capture — is what to do about
    // the walls and the garrison. A board that drops them cannot pose it.
    let cities: Vec<Held> = g
        .cities
        .values()
        .filter(|held| held.owner == a || held.owner == b)
        .filter(|held| g.wdist(held.pos, centre) <= radius)
        .filter_map(|held| {
            let (col, row) = cell(held.pos)?;
            Some(Held {
                role: usize::from(held.owner == b),
                col,
                row,
                hp: held.hp,
                wall_hp: held.wall_hp.max(0),
                name: held.name.clone(),
            })
        })
        .collect();
    let rules = &g.rules;
    let describe = |pid: usize, force: &[Placed]| {
        let material: f64 = force
            .iter()
            .map(|unit| {
                rules
                    .units
                    .get(unit.kind.as_str())
                    .map_or(0.0, |spec| spec.cost)
            })
            .sum();
        format!(
            "seat {pid} ({}): {} units, {material:.0} material",
            g.players[pid].civ,
            force.len()
        )
    };
    Some(Engagement {
        id: id.to_string(),
        name: format!("turn {} contact, seat {a} v seat {b}", g.turn),
        provenance: format!(
            "captured from a {}-player game, seed {}, turn {}",
            g.players.len(),
            g.seed,
            g.turn
        ),
        problem: "The fight the controller actually got into, on the ground it got into it on, \
                  with the army it brought."
            .to_string(),
        roles: [describe(a, &forces[0]), describe(b, &forces[1])],
        width,
        height,
        turns,
        terrain,
        rivers,
        cities,
        forces,
        heal: false,
    })
}

/// The world a harvest plays and the window it captures.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Harvest {
    pub players: usize,
    pub width: i32,
    pub height: i32,
    /// The last turn played.
    pub turns: u32,
    /// Tiles around the contact that make the board.
    pub radius: i32,
    /// The clock each captured board is read on.
    pub window_turns: u32,
    /// Turns before the same pair of players is captured again.
    pub cooldown: u32,
    /// Only engagements between two major civilizations. The barbarian seat
    /// produces most contacts on most maps — a raid is the engagement the
    /// live seat loses the most units to — but a file of wars is a
    /// different distribution from a file of raids, and a run should say
    /// which it is reading.
    pub majors_only: bool,
}

impl Default for Harvest {
    fn default() -> Self {
        Harvest {
            players: 4,
            width: 60,
            height: 38,
            turns: 150,
            radius: 7,
            window_turns: 16,
            cooldown: 25,
            majors_only: false,
        }
    }
}

/// Play one world game with the deployed controller in every seat and take
/// every engagement it produces: the first turn each pair of players at war
/// has armies within [`CONTACT_RANGE`], and again once `cooldown` turns have
/// passed for that pair — a long war is several engagements, a skirmish that
/// never breaks contact is one. The barbarian seat is a player like any
/// other here: a raid is an engagement too, and the one the live seat loses
/// most units to.
pub fn harvest_engagements(seed: u64, setup: &Harvest) -> Vec<Engagement> {
    let Harvest {
        players,
        width,
        height,
        turns,
        radius,
        window_turns,
        cooldown,
        majors_only,
    } = *setup;
    let mut g = Game::new_with(crate::game::GameOptions::new(
        players,
        width,
        height,
        seed,
        turns + 1,
        0,
    ));
    let mut ais = crate::ai::AdvancedAi::fleet(&g);
    g.set_fog_memory(false);
    g.set_war_ledger(false);
    let mut taken: BTreeMap<(usize, usize), u32> = BTreeMap::new();
    let mut boards = Vec::new();
    let mut last_turn = g.turn;
    while g.winner.is_none() && g.turn <= turns {
        let pid = g.current;
        if pid < ais.len() {
            ais[pid].take_turn(&mut g, pid);
        }
        if g.winner.is_none() && g.current == pid {
            let _ = g.apply(pid, &crate::game::Action::EndTurn);
        }
        if g.turn == last_turn {
            continue;
        }
        last_turn = g.turn;
        let seats = g.players.len();
        for a in 0..seats {
            for b in (a + 1)..seats {
                if !g.players[a].alive || !g.players[b].alive || !g.is_at_war(a, b) {
                    continue;
                }
                if majors_only
                    && [a, b]
                        .iter()
                        .any(|seat| g.players[*seat].is_barbarian || g.players[*seat].is_minor)
                {
                    continue;
                }
                if taken
                    .get(&(a, b))
                    .is_some_and(|since| g.turn.saturating_sub(*since) < cooldown)
                {
                    continue;
                }
                let id = format!("s{seed}-t{}-p{a}v{b}", g.turn);
                if let Some(board) = capture_engagement(&g, a, b, radius, window_turns, &id) {
                    boards.push(board);
                    taken.insert((a, b), g.turn);
                }
            }
        }
    }
    boards
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
        let variance = differences.iter().map(|d| (d - mean).powi(2)).sum::<f64>() / (n - 1) as f64;
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
mod tests;
