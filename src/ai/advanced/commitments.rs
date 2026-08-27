//! Commitments: every multi-turn decision the controller makes, tracked to
//! completion, and what became of each. See `docs/COMMITMENTS.md`.
//!
//! The ledger is an **observer at the turn boundary**. At the end of every
//! acting turn it reads the target maps the controller already keeps —
//! `settler_targets`, `builder_targets`, the war plan's objective city —
//! compares them with what it saw the turn before, and classifies what became
//! of each decision: completed, retargeted, dropped, lost with its owner, or
//! still open — and for an open one, whether the owner acted on it this turn
//! (a decision nobody acts on is **forgotten**), whether acting got it any
//! closer (not for [`STALL_TURNS`] turns is **stalled**), and whether it is
//! past the ETA it was priced at when it was made (**late**).
//!
//! It changes no decision. A genome with the ledger on and off plays
//! byte-identical games, which is what makes its counts a measurement rather
//! than a claim. Genes that *act* on the ledger are priced on the screen like
//! any other; the ledger itself is infrastructure and always runs.
//!
//! Why a counter and not a lock: the repository already priced "commit to the
//! strategy and stop changing it" (2026-07-31, `docs/AI_GAPS.md`) and it
//! regressed. The operator's complaint is about the carrying-out, and that is
//! what this counts.

use std::collections::{BTreeMap, BTreeSet};

use crate::game::Game;
use crate::name::Name;
use crate::think;
use crate::Pos;

use super::{AdvancedAi, GrandStrategy, WarPhase};

/// Turns without a better progress reading before an open commitment counts
/// as stalled. Three is `SETTLER_STALL_LIMIT`'s value, so the two readings
/// agree on what "not getting there" means.
pub const STALL_TURNS: u32 = 3;

/// A declared war whose objective has no own military unit within this many
/// hexes is a war nobody is prosecuting this turn.
pub const CAPTURE_PRESENCE_RADIUS: i32 = 3;

/// A settler that vanished without a city at its site, but with a new city of
/// ours this close to where it stood, founded somewhere else.
const SETTLED_ELSEWHERE_RADIUS: i32 = 3;

/// `capture-go-or-stand-down`: consecutive declared-war turns with nobody of
/// ours within [`CAPTURE_PRESENCE_RADIUS`] of the objective before the gene
/// stands the target down. Six is longer than any march the war plan prices
/// for a target it appointed (`estimated_march_turns` is bounded by the
/// package gate), so a streak this long is a target no army is going to.
pub const CAPTURE_GO_TURNS: u32 = 6;

/// How long a stood-down city stays out of the target ranking. Twenty is
/// [`CONQUEST_ETA_TURNS`]: the same patience the campaign layer gives a plan.
pub const CAPTURE_STAND_DOWN_TURNS: u32 = 20;

/// `commitment-patience`: consecutive forgotten turns a settle or improve
/// commitment survives before the ledger retires it — the target dropped and
/// parked in the owner's avoid map for the hysteresis window, so the next
/// pick is a different one. Three is [`STALL_TURNS`] and
/// `SETTLER_STALL_LIMIT`: the controller's own definition of "not getting
/// there". A passing raider moves on inside it; a camp guard does not.
pub const COMMITMENT_PATIENCE: u32 = 3;

/// The terrain walk price looks this far; a target beyond it is priced at a
/// hex a turn, the pessimistic reading.
const WALK_PRICE_RADIUS: i32 = 16;

/// The ETA a Conquest target carries when no appointed war priced one: the
/// campaign layer's own patience (`CAMPAIGN_PATIENCE`), the longest the
/// controller lets a planned city wait before it drops the plan.
pub const CONQUEST_ETA_TURNS: u32 = 20;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Kind {
    /// A Settler bound for a site (`settler_targets`).
    Settle,
    /// A Builder bound for a tile to improve (`builder_targets`).
    Improve,
    /// The appointed war's objective city (`war_plan.objective_city`).
    Capture,
}

impl Kind {
    pub const ALL: [Kind; 3] = [Kind::Settle, Kind::Improve, Kind::Capture];

    pub fn as_str(self) -> &'static str {
        match self {
            Kind::Settle => "settle",
            Kind::Improve => "improve",
            Kind::Capture => "capture",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Owner {
    Unit(u32),
    Empire,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Target {
    Tile(Pos),
    City(u32),
}

/// One open decision.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Commitment {
    pub kind: Kind,
    pub owner: Owner,
    pub target: Target,
    /// Turn the decision was first observed.
    pub made: u32,
    /// Turn it was expected to complete, priced when it was made.
    pub eta: u32,
    /// The progress reading when the decision was made.
    pub initial: i32,
    /// Best (lowest) progress reading so far: hexes to walk for a unit,
    /// phase-and-hit-points for a capture.
    pub best: i32,
    pub best_turn: u32,
    /// Where the owner stood at the last reading (units only).
    pub last_pos: Option<Pos>,
    /// What stood on the target tile when the decision was made (Improve).
    pub improvement_then: Option<Name>,
    pub retargets: u32,
    /// Turns this commitment has been observed open.
    pub turns_open: u32,
    pub forgotten_turns: u32,
    /// Consecutive forgotten turns as of the last reading; a turn the owner
    /// acted on resets it, a turn with no reading leaves it.
    pub forgotten_streak: u32,
    pub stalled_turns: u32,
}

/// What became of one kind of decision, summed over a seat or a board.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct KindCensus {
    pub made: u32,
    pub completed: u32,
    /// The owner was pointed at a different target before completing.
    pub retargeted: u32,
    /// The decision was dropped with the owner alive (a target drop, a
    /// stand-down).
    pub abandoned: u32,
    /// The owner died or was captured with the decision open.
    pub lost: u32,
    /// Commitment-turns observed open — the denominator of the three below.
    pub open_turns: u32,
    /// Open, owner alive with movement to spend, and it did not act.
    pub forgotten_turns: u32,
    /// Open and acted on, but no closer than [`STALL_TURNS`] turns ago.
    pub stalled_turns: u32,
    /// Open past the ETA it was priced at.
    pub late_turns: u32,
    /// Sum over completions of (completed turn − made turn).
    pub completion_turns: u32,
    /// Sum over completions of (eta − made turn): what those took against
    /// what they were priced at.
    pub eta_turns: u32,
}

impl KindCensus {
    pub fn absorb(&mut self, other: &KindCensus) {
        self.made += other.made;
        self.completed += other.completed;
        self.retargeted += other.retargeted;
        self.abandoned += other.abandoned;
        self.lost += other.lost;
        self.open_turns += other.open_turns;
        self.forgotten_turns += other.forgotten_turns;
        self.stalled_turns += other.stalled_turns;
        self.late_turns += other.late_turns;
        self.completion_turns += other.completion_turns;
        self.eta_turns += other.eta_turns;
    }

    /// Decisions that reached an ending of any kind.
    pub fn resolved(&self) -> u32 {
        self.completed + self.retargeted + self.abandoned + self.lost
    }

    /// One line, the way `audit` and the census print it.
    pub fn line(&self) -> String {
        let pct = |part: u32, whole: u32| (100 * part).checked_div(whole).unwrap_or(0);
        let mean = |sum: u32, n: u32| {
            if n == 0 {
                0.0
            } else {
                sum as f64 / n as f64
            }
        };
        format!(
            "made {} done {} ({}%) retargeted {} dropped {} lost {} · open turns {}: forgotten {} ({}%) stalled {} ({}%) late {} ({}%) · done in {:.1} turns v eta {:.1}",
            self.made,
            self.completed,
            pct(self.completed, self.resolved()),
            self.retargeted,
            self.abandoned,
            self.lost,
            self.open_turns,
            self.forgotten_turns,
            pct(self.forgotten_turns, self.open_turns),
            self.stalled_turns,
            pct(self.stalled_turns, self.open_turns),
            self.late_turns,
            pct(self.late_turns, self.open_turns),
            mean(self.completion_turns, self.completed),
            mean(self.eta_turns, self.completed),
        )
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CommitmentCensus {
    pub settle: KindCensus,
    pub improve: KindCensus,
    pub capture: KindCensus,
}

impl CommitmentCensus {
    pub fn get(&self, kind: Kind) -> KindCensus {
        match kind {
            Kind::Settle => self.settle,
            Kind::Improve => self.improve,
            Kind::Capture => self.capture,
        }
    }

    fn slot(&mut self, kind: Kind) -> &mut KindCensus {
        match kind {
            Kind::Settle => &mut self.settle,
            Kind::Improve => &mut self.improve,
            Kind::Capture => &mut self.capture,
        }
    }

    pub fn absorb(&mut self, other: &CommitmentCensus) {
        self.settle.absorb(&other.settle);
        self.improve.absorb(&other.improve);
        self.capture.absorb(&other.capture);
    }

    /// Every kind summed: the seat's decisions as one number each.
    pub fn total(&self) -> KindCensus {
        let mut total = KindCensus::default();
        for kind in Kind::ALL {
            total.absorb(&self.get(kind));
        }
        total
    }

    /// One line per kind, in [`Kind::ALL`] order.
    pub fn lines(&self) -> Vec<String> {
        Kind::ALL
            .iter()
            .map(|kind| format!("{:<8}{}", kind.as_str(), self.get(*kind).line()))
            .collect()
    }
}

#[derive(Clone, Debug, Default)]
pub struct CommitmentLedger {
    open: BTreeMap<(Kind, Owner), Commitment>,
    pub census: CommitmentCensus,
    /// How commitments ended, by kind and the reason the observer can tell
    /// from outside: `completed`, `retargeted`, `dropped`, `lost`,
    /// `settled elsewhere`, `stood down`.
    pub endings: BTreeMap<(Kind, &'static str), u32>,
    /// Why a committed unit with movement did not act, by kind and the hold
    /// the observer can see from outside — the split that says which gene
    /// a forgotten turn wants.
    pub forgotten_why: BTreeMap<(Kind, &'static str), u32>,
    /// Our city ids at the last reading, so a city that is new this turn can
    /// be told from one that was already there.
    cities_seen: BTreeSet<u32>,
}

impl CommitmentLedger {
    pub fn open(&self) -> impl Iterator<Item = &Commitment> {
        self.open.values()
    }

    pub fn open_for(&self, kind: Kind, owner: Owner) -> Option<&Commitment> {
        self.open.get(&(kind, owner))
    }

    fn end(&mut self, commitment: Commitment, how: &'static str, turn: u32) {
        let census = self.census.slot(commitment.kind);
        match how {
            "completed" => {
                census.completed += 1;
                census.completion_turns += turn.saturating_sub(commitment.made);
                census.eta_turns += commitment.eta.saturating_sub(commitment.made);
            }
            "retargeted" => census.retargeted += 1,
            "lost" => census.lost += 1,
            _ => census.abandoned += 1,
        }
        *self.endings.entry((commitment.kind, how)).or_default() += 1;
    }

    fn observe_open(
        &mut self,
        key: (Kind, Owner),
        turn: u32,
        reading: i32,
        acted: Option<bool>,
        pos: Option<Pos>,
        why: &'static str,
    ) {
        let Some(c) = self.open.get_mut(&key) else {
            return;
        };
        let census = self.census.slot(key.0);
        census.open_turns += 1;
        c.turns_open += 1;
        if reading < c.best {
            c.best = reading;
            c.best_turn = turn;
        }
        match acted {
            Some(false) => {
                c.forgotten_turns += 1;
                c.forgotten_streak += 1;
                census.forgotten_turns += 1;
                *self.forgotten_why.entry((key.0, why)).or_default() += 1;
            }
            Some(true) if turn.saturating_sub(c.best_turn) >= STALL_TURNS => {
                c.forgotten_streak = 0;
                c.stalled_turns += 1;
                census.stalled_turns += 1;
            }
            Some(true) => c.forgotten_streak = 0,
            None => {}
        }
        if turn > c.eta {
            census.late_turns += 1;
        }
        if pos.is_some() {
            c.last_pos = pos;
        }
    }

    fn open_new(&mut self, commitment: Commitment) {
        self.census.slot(commitment.kind).made += 1;
        self.open
            .insert((commitment.kind, commitment.owner), commitment);
    }

    /// Test-only flat-ground ETA baseline: two hexes a turn, rounded up.
    /// Production pricing follows the terrain walk in
    /// `AdvancedAi::reconcile_commitments` instead.
    #[cfg(test)]
    fn walk_eta(turn: u32, hexes: i32) -> u32 {
        turn + (hexes.max(0) as u32).div_ceil(2)
    }

    /// `commitment-patience`: a commitment the gene gave up on. Ends as
    /// `retired`, counted with the abandoned.
    pub(super) fn retire(&mut self, key: (Kind, Owner), turn: u32) -> bool {
        match self.open.remove(&key) {
            Some(c) => {
                self.end(c, "retired", turn);
                true
            }
            None => false,
        }
    }

    /// The unit kinds: compare last turn's openings with the map as it stands
    /// at the end of this turn.
    fn reconcile_units(
        &mut self,
        g: &Game,
        pid: usize,
        kind: Kind,
        now: &BTreeMap<u32, Pos>,
        ctx: &UnitContext<'_>,
    ) {
        let UnitContext {
            new_cities,
            holds,
            price,
        } = *ctx;
        let turn = g.turn;
        let gone: Vec<Commitment> = self
            .open
            .iter()
            .filter(|((k, owner), c)| {
                *k == kind
                    && match owner {
                        Owner::Unit(uid) => {
                            now.get(uid).map(|site| Target::Tile(*site)) != Some(c.target)
                        }
                        Owner::Empire => false,
                    }
            })
            .map(|(_, c)| c.clone())
            .collect();
        for c in gone {
            let (Owner::Unit(uid), Target::Tile(site)) = (c.owner, c.target) else {
                continue;
            };
            self.open.remove(&(kind, c.owner));
            let alive = g.units.contains_key(&uid);
            let done = match kind {
                Kind::Settle => g
                    .city_at(site)
                    .and_then(|cid| g.cities.get(&cid))
                    .is_some_and(|city| city.owner == pid),
                Kind::Improve => g
                    .map
                    .get(site)
                    .is_some_and(|tile| tile.improvement != c.improvement_then),
                Kind::Capture => false,
            };
            let how = if done {
                "completed"
            } else if now.contains_key(&uid) {
                "retargeted"
            } else if alive {
                "dropped"
            } else if kind == Kind::Settle
                && new_cities.iter().any(|pos| {
                    *pos != site
                        && c.last_pos
                            .is_some_and(|last| g.wdist(last, *pos) <= SETTLED_ELSEWHERE_RADIUS)
                })
            {
                "settled elsewhere"
            } else {
                "lost"
            };
            let retargets = c.retargets;
            if how == "retargeted" {
                let split = if c.best < c.initial {
                    "retargeted en route"
                } else {
                    "retargeted before moving"
                };
                *self.endings.entry((kind, split)).or_default() += 1;
            }
            self.end(c, how, turn);
            if how == "retargeted" {
                if let (Some(site), Some(unit)) = (now.get(&uid), g.units.get(&uid)) {
                    let hexes = g.wdist(unit.pos, *site);
                    self.open_new(Commitment {
                        kind,
                        owner: Owner::Unit(uid),
                        target: Target::Tile(*site),
                        made: turn,
                        eta: turn + price(uid, *site, hexes),
                        initial: hexes,
                        best: hexes,
                        best_turn: turn,
                        last_pos: Some(unit.pos),
                        improvement_then: g.map.get(*site).and_then(|tile| tile.improvement),
                        retargets: retargets + 1,
                        turns_open: 0,
                        forgotten_turns: 0,
                        forgotten_streak: 0,
                        stalled_turns: 0,
                    });
                }
            }
        }
        for (&uid, &site) in now {
            let Some(unit) = g.units.get(&uid) else {
                continue;
            };
            let key = (kind, Owner::Unit(uid));
            let hexes = g.wdist(unit.pos, site);
            if self.open.contains_key(&key) {
                // A Builder that improved its pinned tile and kept the pin is
                // done, whatever the pin says next turn.
                if kind == Kind::Improve
                    && self.open[&key].improvement_then
                        != g.map.get(site).and_then(|tile| tile.improvement)
                {
                    let c = self.open.remove(&key).unwrap();
                    self.end(c, "completed", turn);
                    continue;
                }
                let acted = unit.acted || unit.moves_left <= 0.0;
                let why = holds.get(&uid).copied().unwrap_or("unexplained");
                self.observe_open(key, turn, hexes, Some(acted), Some(unit.pos), why);
            } else {
                self.open_new(Commitment {
                    kind,
                    owner: Owner::Unit(uid),
                    target: Target::Tile(site),
                    made: turn,
                    eta: turn + price(uid, site, hexes),
                    initial: hexes,
                    best: hexes,
                    best_turn: turn,
                    last_pos: Some(unit.pos),
                    improvement_then: g.map.get(site).and_then(|tile| tile.improvement),
                    retargets: 0,
                    turns_open: 0,
                    forgotten_turns: 0,
                    forgotten_streak: 0,
                    stalled_turns: 0,
                });
            }
        }
    }

    /// The city the empire means to take: the appointed war's objective when
    /// one is appointed, otherwise the grand strategy's Conquest target. One
    /// commitment per empire.
    fn reconcile_capture(&mut self, g: &Game, pid: usize, war: Option<CaptureReading>) {
        let turn = g.turn;
        let key = (Kind::Capture, Owner::Empire);
        if let Some(c) = self.open.get(&key) {
            let same = war
                .as_ref()
                .is_some_and(|w| Target::City(w.city) == c.target);
            if !same {
                let Target::City(cid) = c.target else {
                    unreachable!("a capture targets a city")
                };
                let ours = g.cities.get(&cid).is_some_and(|city| city.owner == pid);
                let c = self.open.remove(&key).unwrap();
                let how = if ours {
                    "completed"
                } else if war.is_some() {
                    "retargeted"
                } else {
                    "stood down"
                };
                if how != "completed" {
                    // Did anyone ever go? A capture that was forgotten on
                    // every one of its open turns was a decision with no
                    // army behind it.
                    let split = if c.turns_open > 0 && c.forgotten_turns == c.turns_open {
                        "ended, nobody ever went"
                    } else if c.forgotten_turns == 0 && c.turns_open > 0 {
                        "ended, always present"
                    } else {
                        "ended, went then left"
                    };
                    *self.endings.entry((Kind::Capture, split)).or_default() += 1;
                }
                self.end(c, how, turn);
            }
        }
        let Some(w) = war else { return };
        if self.open.contains_key(&key) {
            // Before the declaration the war is the empire's build queue, and
            // whether the phase advances is the only reading; from the
            // declaration on, presence at the objective and its hit points are.
            let acted = w.declared.then_some(w.present);
            self.observe_open(key, turn, w.reading, acted, None, "nobody at the objective");
        } else {
            self.open_new(Commitment {
                kind: Kind::Capture,
                owner: Owner::Empire,
                target: Target::City(w.city),
                made: turn,
                eta: w.eta,
                initial: w.reading,
                best: w.reading,
                best_turn: turn,
                last_pos: None,
                improvement_then: None,
                retargets: 0,
                turns_open: 0,
                forgotten_turns: 0,
                forgotten_streak: 0,
                stalled_turns: 0,
            });
        }
    }

    /// Write the running census into the seat's counters so a screen row
    /// carries it. Keys are `commit:<kind>:<field>` per kind and
    /// `commit:<field>` summed. Overwrites, so the value is the seat's total.
    fn export(&self, g: &mut Game, pid: usize) {
        let counters = &mut g.players[pid].counters;
        let mut put = |key: String, value: u32| {
            let value = i64::from(value);
            match counters.get_mut(&key) {
                Some(slot) => *slot = value,
                None => {
                    counters.insert(key, value);
                }
            }
        };
        let fields = |c: KindCensus| {
            [
                ("made", c.made),
                ("completed", c.completed),
                ("retargeted", c.retargeted),
                ("abandoned", c.abandoned),
                ("lost", c.lost),
                ("open_turns", c.open_turns),
                ("forgotten_turns", c.forgotten_turns),
                ("stalled_turns", c.stalled_turns),
                ("late_turns", c.late_turns),
                ("completion_turns", c.completion_turns),
                ("eta_turns", c.eta_turns),
            ]
        };
        for kind in Kind::ALL {
            for (name, value) in fields(self.census.get(kind)) {
                put(format!("commit:{}:{name}", kind.as_str()), value);
            }
        }
        for (name, value) in fields(self.census.total()) {
            put(format!("commit:{name}"), value);
        }
    }
}

/// What one turn's reading of the unit kinds needs beside the target map.
struct UnitContext<'a> {
    /// Our cities that are new this turn (founded or taken).
    new_cities: &'a [Pos],
    /// Why each idle committed unit did not act, by unit id.
    holds: &'a BTreeMap<u32, &'static str>,
    /// Turns a new decision is priced at: (unit, site, hexes) → turns.
    price: &'a dyn Fn(u32, Pos, i32) -> u32,
}

/// The war plan as the ledger reads it.
struct CaptureReading {
    city: u32,
    eta: u32,
    /// Lower is closer: phases still to go, then the city's hit points and
    /// walls.
    reading: i32,
    declared: bool,
    /// An own military unit stands within [`CAPTURE_PRESENCE_RADIUS`].
    present: bool,
}

impl AdvancedAi {
    /// The end-of-turn reading. Called once per acting turn, after the unit
    /// pass and before `EndTurn`, so `Unit::acted` still says what each unit
    /// did this turn.
    pub(super) fn reconcile_commitments(&mut self, g: &mut Game, pid: usize) {
        let new_cities: Vec<Pos> = g
            .cities
            .values()
            .filter(|city| city.owner == pid && !self.commitments.cities_seen.contains(&city.id))
            .map(|city| city.pos)
            .collect();
        let presence = |g: &Game, at: Pos| {
            g.units.values().any(|unit| {
                unit.owner == pid
                    && g.rules.units[unit.kind].class == "military"
                    && g.wdist(unit.pos, at) <= CAPTURE_PRESENCE_RADIUS
            })
        };
        let appointed = self.war_plan.as_ref().and_then(|plan| {
            let city = g.cities.get(&plan.objective_city)?;
            let phases_to_go = match plan.phase {
                WarPhase::Research => 4,
                WarPhase::Mobilize => 3,
                WarPhase::Stage => 2,
                WarPhase::Strike => 1,
                WarPhase::Exploit => 0,
            };
            let declared = plan.declared_turn.is_some();
            Some(CaptureReading {
                city: plan.objective_city,
                eta: plan.appointed_turn
                    + plan.estimated_research_turns
                    + plan.estimated_production_turns
                    + plan.estimated_march_turns,
                reading: phases_to_go * 1000 + city.hp + city.wall_hp,
                declared,
                present: declared && presence(g, city.pos),
            })
        });
        let conquest = self.plan.as_ref().and_then(|plan| {
            if plan.strategy != GrandStrategy::Conquest {
                return None;
            }
            let cid = plan.target_city?;
            let city = g.cities.get(&cid)?;
            if city.owner == pid {
                return None;
            }
            let declared = g.is_at_war(pid, city.owner);
            let eta = self
                .commitments
                .open_for(Kind::Capture, Owner::Empire)
                .filter(|c| c.target == Target::City(cid))
                .map(|c| c.eta)
                .unwrap_or(g.turn + CONQUEST_ETA_TURNS);
            Some(CaptureReading {
                city: cid,
                eta,
                reading: if declared { 0 } else { 1000 } + city.hp + city.wall_hp,
                declared,
                present: declared && presence(g, city.pos),
            })
        });
        let war = appointed.or(conquest);
        // Why a committed unit with movement to spend did not act, from what
        // the controller's own maps and the board say. Only the first hold
        // that applies is named, in this order.
        let hostile_near = |g: &Game, at: Pos| {
            g.units.values().any(|unit| {
                unit.owner != pid
                    && g.is_at_war(pid, unit.owner)
                    && g.rules.units[unit.kind].class == "military"
                    && g.wdist(unit.pos, at) <= 2
            })
        };
        let in_own_city = |g: &Game, at: Pos| {
            g.city_at(at)
                .and_then(|cid| g.cities.get(&cid))
                .is_some_and(|city| city.owner == pid)
        };
        let idle = |g: &Game, uid: u32| {
            g.units
                .get(&uid)
                .is_some_and(|unit| !unit.acted && unit.moves_left > 0.0)
        };
        let mut holds: BTreeMap<u32, &'static str> = BTreeMap::new();
        for (&uid, &site) in &self.settler_targets {
            if !idle(g, uid) {
                continue;
            }
            let at = g.units[&uid].pos;
            let why = if self.guard_wait.contains_key(&uid) {
                "waiting for escort"
            } else if self.settler_threat_deferrals.contains_key(&site) {
                "threat forecast on the site"
            } else if hostile_near(g, at) {
                "hostile within two"
            } else if self
                .settler_stalls
                .get(&uid)
                .is_some_and(|stalls| *stalls > 0)
            {
                "stall counted (route refused)"
            } else if in_own_city(g, at) {
                "in a city"
            } else {
                "unexplained"
            };
            holds.insert(uid, why);
        }
        for (&uid, &site) in &self.builder_targets {
            if !idle(g, uid) {
                continue;
            }
            let at = g.units[&uid].pos;
            let why = if hostile_near(g, at) {
                "hostile within two"
            } else if at == site {
                "at the tile, build refused"
            } else if in_own_city(g, at) {
                "in a city"
            } else {
                "walk refused or not attempted"
            };
            holds.insert(uid, why);
        }
        // The ETA a new decision carries: the terrain walk in turns
        // (`settle_sooner_walk_costs`, movement points over `step_cost`), at
        // the owner's allowance; a hex a turn when the walk cannot reach the
        // target inside the radius. Priced once, when the decision is made.
        let price = |uid: u32, site: Pos, hexes: i32| -> u32 {
            let Some(unit) = g.units.get(&uid) else {
                return (hexes.max(0) as u32).div_ceil(2);
            };
            let moves = g.rules.units[unit.kind].moves.max(1.0);
            let radius = (hexes + 3).min(WALK_PRICE_RADIUS);
            match AdvancedAi::settle_sooner_walk_costs(g, uid, radius).get(&site) {
                Some(cost) => (cost / moves).ceil().max(1.0) as u32,
                None => hexes.max(1) as u32,
            }
        };
        let ctx = UnitContext {
            new_cities: &new_cities,
            holds: &holds,
            price: &price,
        };
        let ledger = &mut self.commitments;
        ledger.reconcile_units(g, pid, Kind::Settle, &self.settler_targets, &ctx);
        ledger.reconcile_units(g, pid, Kind::Improve, &self.builder_targets, &ctx);
        ledger.reconcile_capture(g, pid, war);
        // `commitment-patience`: a settle or improve decision forgotten for
        // COMMITMENT_PATIENCE turns running is retired — target dropped and
        // parked for the hysteresis window — instead of held for ever.
        if self.commitment_patience {
            self.builder_avoid.retain(|_, (_, until)| g.turn < *until);
            let expired: Vec<(Kind, u32, Pos)> = self
                .commitments
                .open()
                .filter(|c| c.forgotten_streak >= COMMITMENT_PATIENCE)
                .filter_map(|c| match (c.kind, c.owner, c.target) {
                    (Kind::Capture, _, _) => None,
                    (kind, Owner::Unit(uid), Target::Tile(site)) => Some((kind, uid, site)),
                    _ => None,
                })
                .collect();
            for (kind, uid, site) in expired {
                let until = g.turn + g.standard_duration(super::SETTLER_TARGET_HYSTERESIS_TURNS);
                match kind {
                    Kind::Settle => {
                        self.settler_targets.remove(&uid);
                        self.settler_dead_sites
                            .entry(uid)
                            .or_default()
                            .insert(site, until);
                    }
                    Kind::Improve => {
                        self.builder_targets.remove(&uid);
                        self.builder_avoid.insert(uid, (site, until));
                    }
                    Kind::Capture => continue,
                }
                self.commitments.retire((kind, Owner::Unit(uid)), g.turn);
                *g.players[pid]
                    .counters
                    .entry(format!("commit:{}:retired", kind.as_str()))
                    .or_insert(0) += 1;
                think!(self.journal(), Expansion, Detail, "Giving up a target nobody is walking to";
                       "{} {uid} held {site:?} for {COMMITMENT_PATIENCE} turns without acting on it; the site is set aside until turn {until}",
                       kind.as_str(); site);
            }
        }
        let ledger = &mut self.commitments;
        // `capture-go-or-stand-down`: the one place the ledger acts. A
        // declared objective forgotten for CAPTURE_GO_TURNS running is stood
        // down: out of the ranking until the stand-down expires, and
        // `plan_stale` re-assesses the strategy next turn.
        if self.capture_go_or_stand_down {
            self.capture_stood_down.retain(|_, until| g.turn < *until);
            if let Some(c) = ledger.open_for(Kind::Capture, Owner::Empire) {
                let Target::City(cid) = c.target else {
                    unreachable!("a capture targets a city")
                };
                if c.forgotten_streak >= CAPTURE_GO_TURNS
                    && !self.capture_stood_down.contains_key(&cid)
                {
                    let (turns_open, streak) = (c.turns_open, c.forgotten_streak);
                    self.capture_stood_down
                        .insert(cid, g.turn + CAPTURE_STAND_DOWN_TURNS);
                    *g.players[pid]
                        .counters
                        .entry("commit:capture:gene_stand_downs".to_string())
                        .or_insert(0) += 1;
                    if let Some(city) = g.cities.get(&cid) {
                        let (name, pos) = (city.name.clone(), city.pos);
                        think!(self.journal(), Military, Strategy, "Standing down the objective nobody went to";
                               "{name} was the target for {turns_open} turns and no unit of ours came within {} hexes on the last {streak}",
                               CAPTURE_PRESENCE_RADIUS; pos);
                    }
                }
            }
        }
        let ledger = &mut self.commitments;
        ledger.cities_seen = g
            .cities
            .values()
            .filter(|city| city.owner == pid)
            .map(|city| city.id)
            .collect();
        ledger.export(g, pid);
    }

    /// `capture-go-or-stand-down`: whether the gene holds this city out of
    /// the target ranking this turn.
    pub(super) fn capture_stood_down_holds(&self, g: &Game, city: u32) -> bool {
        self.capture_go_or_stand_down
            && self
                .capture_stood_down
                .get(&city)
                .is_some_and(|until| g.turn < *until)
    }

    /// What became of this seat's decisions so far.
    pub fn commitment_census(&self) -> CommitmentCensus {
        self.commitments.census
    }

    /// The ledger itself, for censuses that want the open set or the endings.
    pub fn commitments(&self) -> &CommitmentLedger {
        &self.commitments
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ledger_with(kind: Kind, uid: u32, site: Pos, turn: u32, hexes: i32) -> CommitmentLedger {
        let mut ledger = CommitmentLedger::default();
        ledger.open_new(Commitment {
            kind,
            owner: Owner::Unit(uid),
            target: Target::Tile(site),
            made: turn,
            eta: CommitmentLedger::walk_eta(turn, hexes),
            initial: hexes,
            best: hexes,
            best_turn: turn,
            last_pos: None,
            improvement_then: None,
            retargets: 0,
            turns_open: 0,
            forgotten_turns: 0,
            forgotten_streak: 0,
            stalled_turns: 0,
        });
        ledger
    }

    #[test]
    fn a_walk_is_priced_at_two_hexes_a_turn_and_never_zero() {
        assert_eq!(CommitmentLedger::walk_eta(10, 0), 10);
        assert_eq!(CommitmentLedger::walk_eta(10, 1), 11);
        assert_eq!(CommitmentLedger::walk_eta(10, 4), 12);
        assert_eq!(CommitmentLedger::walk_eta(10, 5), 13);
    }

    #[test]
    fn an_unacted_turn_is_forgotten_and_a_fruitless_one_is_stalled_only_after_the_limit() {
        let mut ledger = ledger_with(Kind::Settle, 7, (5, 5), 10, 6);
        let key = (Kind::Settle, Owner::Unit(7));
        ledger.observe_open(key, 11, 6, Some(false), Some((1, 1)), "test");
        assert_eq!(ledger.census.settle.forgotten_turns, 1);
        assert_eq!(ledger.census.settle.stalled_turns, 0);
        // Acting without getting closer: stalled once STALL_TURNS have passed
        // since the best reading, not before.
        ledger.observe_open(key, 12, 6, Some(true), Some((1, 1)), "test");
        assert_eq!(ledger.census.settle.stalled_turns, 0);
        ledger.observe_open(key, 13, 6, Some(true), Some((1, 1)), "test");
        assert_eq!(ledger.census.settle.stalled_turns, 1);
        // Getting closer resets the clock.
        ledger.observe_open(key, 14, 4, Some(true), Some((2, 2)), "test");
        assert_eq!(ledger.census.settle.stalled_turns, 1);
        ledger.observe_open(key, 15, 4, Some(true), Some((2, 2)), "test");
        assert_eq!(ledger.census.settle.stalled_turns, 1);
        // ETA was turn 13; turns 14 and 15 are late.
        assert_eq!(ledger.census.settle.late_turns, 2);
        assert_eq!(ledger.census.settle.open_turns, 5);
    }

    #[test]
    fn a_completion_records_its_turns_against_the_eta() {
        let mut ledger = ledger_with(Kind::Improve, 3, (2, 2), 20, 3);
        let c = ledger
            .open
            .remove(&(Kind::Improve, Owner::Unit(3)))
            .unwrap();
        ledger.end(c, "completed", 26);
        let census = ledger.census.improve;
        assert_eq!(
            (census.completed, census.completion_turns, census.eta_turns),
            (1, 6, 2)
        );
        assert_eq!(ledger.endings[&(Kind::Improve, "completed")], 1);
        assert!(
            census.line().contains("done in 6.0 turns v eta 2.0"),
            "{}",
            census.line()
        );
    }

    #[test]
    fn a_capture_before_the_declaration_cannot_be_forgotten() {
        let mut ledger = CommitmentLedger::default();
        ledger.open_new(Commitment {
            kind: Kind::Capture,
            owner: Owner::Empire,
            target: Target::City(9),
            made: 50,
            eta: 80,
            initial: 4200,
            best: 4200,
            best_turn: 50,
            last_pos: None,
            improvement_then: None,
            retargets: 0,
            turns_open: 0,
            forgotten_turns: 0,
            forgotten_streak: 0,
            stalled_turns: 0,
        });
        let key = (Kind::Capture, Owner::Empire);
        for turn in 51..60 {
            ledger.observe_open(key, turn, 4200, None, None, "test");
        }
        assert_eq!(ledger.census.capture.forgotten_turns, 0);
        assert_eq!(ledger.census.capture.stalled_turns, 0);
        ledger.observe_open(key, 60, 1200, Some(false), None, "test");
        assert_eq!(ledger.census.capture.forgotten_turns, 1);
    }

    #[test]
    fn the_total_sums_every_kind_and_lines_name_them() {
        let mut census = CommitmentCensus::default();
        census.settle.made = 2;
        census.improve.made = 3;
        census.capture.made = 1;
        assert_eq!(census.total().made, 6);
        let lines = census.lines();
        assert_eq!(lines.len(), 3);
        assert!(lines[0].starts_with("settle  made 2"));
        assert!(lines[2].starts_with("capture made 1"));
    }

    #[test]
    fn a_forgotten_streak_counts_consecutive_turns_and_an_acted_turn_resets_it() {
        let mut ledger = ledger_with(Kind::Settle, 7, (5, 5), 10, 6);
        let key = (Kind::Settle, Owner::Unit(7));
        ledger.observe_open(key, 11, 6, Some(false), None, "test");
        ledger.observe_open(key, 12, 6, Some(false), None, "test");
        assert_eq!(ledger.open_for(key.0, key.1).unwrap().forgotten_streak, 2);
        ledger.observe_open(key, 13, 6, None, None, "test");
        assert_eq!(
            ledger.open_for(key.0, key.1).unwrap().forgotten_streak,
            2,
            "no reading leaves it"
        );
        ledger.observe_open(key, 14, 5, Some(true), None, "test");
        assert_eq!(ledger.open_for(key.0, key.1).unwrap().forgotten_streak, 0);
        assert_eq!(ledger.open_for(key.0, key.1).unwrap().forgotten_turns, 2);
    }

    /// Two rival cities, a war, and a Conquest plan aimed at the farther one
    /// with no unit of ours anywhere near it. With the gene on, six
    /// unprosecuted turns stand the city down: it leaves the ranking, the
    /// plan is stale at once, and the re-assessment names something else.
    /// With the gene off the ledger only counts, and nothing is written.
    #[test]
    fn a_declared_objective_nobody_goes_to_is_stood_down_and_the_ranking_moves_on() {
        use super::super::StrategicPlan;
        use crate::game::Action;

        let fixture = || {
            let mut game = Game::new_full(2, 24, 16, 5_150, 200, 0, false);
            let settler = game
                .player_unit_ids(0)
                .into_iter()
                .find(|id| game.units[id].kind == "settler")
                .expect("a starting settler");
            game.apply(0, &Action::FoundCity { unit: settler }).unwrap();
            let rival_settler = game
                .player_unit_ids(1)
                .into_iter()
                .find(|id| game.units[id].kind == "settler")
                .expect("a rival settler");
            let first = game.units[&rival_settler].pos;
            game.found_city_for(1, first, None);
            let second = game
                .wdisk(first, 7)
                .into_iter()
                .find(|pos| {
                    game.wdist(*pos, first) >= 5
                        && !game.rules.is_water(&game.map.tiles[pos])
                        && game.units_at(*pos).is_empty()
                        && game.city_at(*pos).is_none()
                })
                .expect("land for a second rival city");
            game.found_city_for(1, second, None);
            game.at_war.insert((0, 1));
            game.at_war.insert((1, 0));
            let ours = game.cities[&game.player_city_ids(0)[0]].pos;
            // Aim at the rival city farthest from us, so our starting warrior
            // is not "present" by accident.
            let target = game
                .player_city_ids(1)
                .into_iter()
                .max_by_key(|cid| game.wdist(game.cities[cid].pos, ours))
                .unwrap();
            assert!(
                game.wdist(game.cities[&target].pos, ours) > CAPTURE_PRESENCE_RADIUS + 1,
                "the fixture needs the objective out of our starting units' reach"
            );
            (game, target)
        };
        let aim = |ai: &mut AdvancedAi, game: &Game, target: u32| {
            ai.plan = Some(StrategicPlan {
                strategy: GrandStrategy::Conquest,
                target_player: Some(1),
                target_city: Some(target),
                threatened_city: None,
                desired_cities: 4,
                assessed_turn: game.turn,
                rush: false,
            });
        };

        let (mut game, target) = fixture();
        let mut ai = AdvancedAi::new();
        ai.enable_capture_go_or_stand_down();
        aim(&mut ai, &game, target);
        // The first boundary registers the decision and takes no reading, so
        // six readings need seven boundaries.
        for _ in 0..=CAPTURE_GO_TURNS {
            ai.reconcile_commitments(&mut game, 0);
            game.turn += 1;
        }
        let open = ai
            .commitments()
            .open_for(Kind::Capture, Owner::Empire)
            .expect("the capture is open");
        assert_eq!(open.forgotten_streak, CAPTURE_GO_TURNS);
        assert!(ai.capture_stood_down.contains_key(&target), "stood down");
        assert!(ai.plan_stale(&game, 0), "re-assess now, not on the cadence");
        assert_eq!(
            game.players[0]
                .counters
                .get("commit:capture:gene_stand_downs"),
            Some(&1)
        );
        let next = ai.assess(&game, 0);
        assert_ne!(
            next.target_city,
            Some(target),
            "the stood-down city is out of the ranking"
        );
        // The stand-down expires.
        game.turn += CAPTURE_STAND_DOWN_TURNS;
        assert!(!ai.capture_stood_down_holds(&game, target));

        let (mut game, target) = fixture();
        let mut off = AdvancedAi::new();
        aim(&mut off, &game, target);
        for _ in 0..=CAPTURE_GO_TURNS {
            off.reconcile_commitments(&mut game, 0);
            game.turn += 1;
        }
        assert!(off.capture_stood_down.is_empty(), "off: nothing written");
        assert_eq!(
            game.players[0]
                .counters
                .get("commit:capture:gene_stand_downs"),
            None
        );
        assert_eq!(
            off.commitments()
                .open_for(Kind::Capture, Owner::Empire)
                .unwrap()
                .forgotten_streak,
            CAPTURE_GO_TURNS,
            "off: the ledger still counts"
        );
    }

    /// `commitment-patience`: a settler and a Builder that never act on their
    /// targets. Three forgotten readings retire both — the targets leave the
    /// maps, the sites are parked, the endings say `retired` — and the gene
    /// off leaves both targets standing however long the unit idles.
    #[test]
    fn a_target_nobody_acts_on_is_retired_after_patience_and_parked() {
        use crate::game::Action;

        let fixture = || {
            let mut game = Game::new_full(2, 24, 16, 5_150, 200, 0, false);
            let settler = game
                .player_unit_ids(0)
                .into_iter()
                .find(|id| game.units[id].kind == "settler")
                .expect("a starting settler");
            game.apply(0, &Action::FoundCity { unit: settler }).unwrap();
            let home = game.cities[&game.player_city_ids(0)[0]].pos;
            let settler = game.spawn_test_unit("settler", 0, home);
            let builder = game.spawn_test_unit("builder", 0, home);
            let land_at = |game: &Game, ring: i32| {
                game.wdisk(home, ring + 1)
                    .into_iter()
                    .filter(|pos| {
                        game.wdist(*pos, home) == ring && !game.rules.is_water(&game.map.tiles[pos])
                    })
                    .min()
                    .expect("a land tile on the ring")
            };
            let far = land_at(&game, 5);
            let near = land_at(&game, 2);
            (game, settler, builder, far, near)
        };
        let idle = |ai: &mut AdvancedAi, game: &mut Game, rounds: u32| {
            for _ in 0..rounds {
                ai.reconcile_commitments(game, 0);
                game.turn += 1;
            }
        };

        let (mut game, settler, builder, far, near) = fixture();
        let mut ai = AdvancedAi::new();
        ai.enable_commitment_patience();
        ai.settler_targets.insert(settler, far);
        ai.builder_targets.insert(builder, near);
        // Registration, then COMMITMENT_PATIENCE forgotten readings.
        idle(&mut ai, &mut game, 1 + COMMITMENT_PATIENCE);
        assert!(
            !ai.settler_targets.contains_key(&settler),
            "the settle target is retired"
        );
        assert!(
            !ai.builder_targets.contains_key(&builder),
            "the improve target is retired"
        );
        assert!(
            ai.settler_site_is_dead(settler, far),
            "the site is parked for this settler"
        );
        assert_eq!(
            ai.builder_avoid.get(&builder).map(|(tile, _)| *tile),
            Some(near)
        );
        assert_eq!(
            ai.commitments().endings.get(&(Kind::Settle, "retired")),
            Some(&1)
        );
        assert_eq!(
            ai.commitments().endings.get(&(Kind::Improve, "retired")),
            Some(&1)
        );
        assert_eq!(ai.commitment_census().settle.abandoned, 1);
        assert_eq!(
            game.players[0].counters.get("commit:settle:retired"),
            Some(&1)
        );

        // A reading short of the patience retires nothing.
        let (mut game, settler, builder, far, near) = fixture();
        let mut early = AdvancedAi::new();
        early.enable_commitment_patience();
        early.settler_targets.insert(settler, far);
        early.builder_targets.insert(builder, near);
        idle(&mut early, &mut game, COMMITMENT_PATIENCE);
        assert!(early.settler_targets.contains_key(&settler));
        assert!(early.builder_targets.contains_key(&builder));

        let (mut game, settler, builder, far, near) = fixture();
        let mut off = AdvancedAi::new();
        off.settler_targets.insert(settler, far);
        off.builder_targets.insert(builder, near);
        idle(&mut off, &mut game, 2 + COMMITMENT_PATIENCE);
        assert!(
            off.settler_targets.contains_key(&settler),
            "off: the target stands"
        );
        assert!(
            off.builder_targets.contains_key(&builder),
            "off: the pin stands"
        );
        assert!(off.builder_avoid.is_empty());
        assert_eq!(
            off.commitments().endings.get(&(Kind::Settle, "retired")),
            None
        );
    }

    /// The ETA is the terrain walk at the unit's allowance, not two hexes a
    /// turn: a five-hex walk for a two-move settler is at least three turns,
    /// and never less than the flat-ground price.
    #[test]
    fn a_new_decision_is_priced_on_the_terrain_walk() {
        use crate::game::Action;

        let mut game = Game::new_full(2, 24, 16, 5_150, 200, 0, false);
        let settler = game
            .player_unit_ids(0)
            .into_iter()
            .find(|id| game.units[id].kind == "settler")
            .expect("a starting settler");
        game.apply(0, &Action::FoundCity { unit: settler }).unwrap();
        let home = game.cities[&game.player_city_ids(0)[0]].pos;
        let settler = game.spawn_test_unit("settler", 0, home);
        let far = game
            .wdisk(home, 6)
            .into_iter()
            .filter(|pos| game.wdist(*pos, home) == 5 && !game.rules.is_water(&game.map.tiles[pos]))
            .min()
            .expect("a land tile five hexes out");
        let mut ai = AdvancedAi::new();
        ai.settler_targets.insert(settler, far);
        ai.reconcile_commitments(&mut game, 0);
        let c = ai
            .commitments()
            .open_for(Kind::Settle, Owner::Unit(settler))
            .expect("registered");
        let priced = c.eta - c.made;
        assert!(
            priced >= 3,
            "five hexes at two moves is three turns or more, got {priced}"
        );
        assert!(
            priced >= CommitmentLedger::walk_eta(0, 5),
            "never below the flat-ground price"
        );
    }

    /// The reading `docs/COMMITMENTS.md` is built on: eight 6-player 60x38
    /// Online maps at the deployment genome, every major's ledger pooled.
    /// `CIVVIS_CENSUS_OPT_INS=tag,tag` switches the named opt-in genes on for
    /// every major, so a gene can be read against the same maps;
    /// `CIVVIS_CENSUS_MAPS` sets the map count (default 8).
    ///
    /// Run with `cargo test --profile ci commitment_census -- --ignored --nocapture`.
    #[test]
    #[ignore = "census, not an assertion; run explicitly with --nocapture"]
    fn commitment_census() {
        use crate::ai::{Ai, GENES};
        use crate::game::{Action, GameOptions};

        let opt_ins: Vec<String> = std::env::var("CIVVIS_CENSUS_OPT_INS")
            .ok()
            .map(|text| {
                text.split(',')
                    .filter(|t| !t.is_empty())
                    .map(str::to_string)
                    .collect()
            })
            .unwrap_or_default();
        let maps = std::env::var("CIVVIS_CENSUS_MAPS")
            .ok()
            .and_then(|text| text.parse::<u64>().ok())
            .unwrap_or(8);
        let mut totals = CommitmentCensus::default();
        let mut endings: BTreeMap<(Kind, &'static str), u32> = BTreeMap::new();
        let mut still_open: BTreeMap<Kind, u32> = BTreeMap::new();
        let mut forgotten_why: BTreeMap<(Kind, &'static str), u32> = BTreeMap::new();
        for map in 0..maps {
            let seed = 98_500_000 + map;
            let mut game = Game::new_with(GameOptions {
                speed: "online".to_string(),
                randomize_civs: true,
                ..GameOptions::new(6, 60, 38, seed, 250, 6)
            });
            game.set_fog_memory(false);
            game.set_war_ledger(false);
            let major = |game: &Game, pid: usize| {
                !game.players[pid].is_minor && !game.players[pid].is_barbarian
            };
            let mut ais: Vec<AdvancedAi> = (0..game.players.len())
                .map(|pid| {
                    let mut ai = AdvancedAi::new();
                    if major(&game, pid) {
                        ai.enable_engine_repairs();
                        for tag in &opt_ins {
                            let enable = GENES
                                .iter()
                                .find(|gene| gene.tag == *tag && gene.opt_in())
                                .unwrap_or_else(|| panic!("{tag} is not an opt-in gene"))
                                .enable;
                            enable(&mut ai);
                        }
                    }
                    ai
                })
                .collect();
            while game.winner.is_none() && game.turn <= game.max_turns {
                let pid = game.current;
                ais[pid].take_turn(&mut game, pid);
                if game.winner.is_none() && game.current == pid {
                    let _ = game.apply(pid, &Action::EndTurn);
                }
            }
            let mut census = CommitmentCensus::default();
            for (pid, ai) in ais.iter().enumerate() {
                if !major(&game, pid) {
                    continue;
                }
                census.absorb(&ai.commitment_census());
                for (key, n) in &ai.commitments().endings {
                    *endings.entry(*key).or_default() += n;
                }
                for c in ai.commitments().open() {
                    *still_open.entry(c.kind).or_default() += 1;
                }
                for (key, n) in &ai.commitments().forgotten_why {
                    *forgotten_why.entry(*key).or_default() += n;
                }
            }
            println!("map {seed} t{}:", game.turn);
            for line in census.lines() {
                println!("  {line}");
            }
            totals.absorb(&census);
        }
        println!(
            "=== {maps} maps, deployment genome{} ===",
            if opt_ins.is_empty() {
                String::new()
            } else {
                format!(" + {}", opt_ins.join(","))
            }
        );
        for line in totals.lines() {
            println!("{line}");
        }
        println!("endings:");
        for ((kind, how), n) in &endings {
            println!("  {:<8}{how:<18}{n}", kind.as_str());
        }
        println!("open at game end:");
        for (kind, n) in &still_open {
            println!("  {:<8}{n}", kind.as_str());
        }
        println!("forgotten, by hold:");
        for ((kind, why), n) in &forgotten_why {
            println!("  {:<8}{why:<32}{n}", kind.as_str());
        }
    }
}
