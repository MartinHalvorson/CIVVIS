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

use crate::game::{Action, Game};
use crate::name::Name;
use crate::reasoning::plain;
use crate::think;
use crate::Pos;

use super::{civilian_safety, AdvancedAi, GrandStrategy, WarPhase};

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

/// `capture-go-or-stand-down-2`: consecutive stalled readings — bodies at the
/// objective, the city not pushed to a new low — before the siege is stood
/// down. A stalled reading starts [`STALL_TURNS`] turns after the last new
/// low, so six of them is nine turns of a siege that is not winning: the
/// arena's `the_storming` board resolves in 34 and a live siege that took a
/// city did it inside ten of the declaration (`TIMED_WAR_CAPTURE_WINDOW`).
pub const CAPTURE_STALL_TURNS: u32 = 6;

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

/// The census fields [`CommitmentLedger::export`] writes under `commit:`;
/// any other `commit:` counter is a ledger-acting gene's own.
const EXPORTED_FIELDS: [&str; 11] = [
    "made",
    "completed",
    "retargeted",
    "abandoned",
    "lost",
    "open_turns",
    "forgotten_turns",
    "stalled_turns",
    "late_turns",
    "completion_turns",
    "eta_turns",
];

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
    /// Last game turn counted; observation refreshes do not age the decision.
    pub observed_turn: u32,
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
    /// Consecutive stalled readings as of the last one; a new low resets it,
    /// a forgotten turn (nobody present) resets it.
    pub stalled_streak: u32,
    /// The reading has been below its initial value at least once: for a
    /// capture, the city was pushed lower than when the decision was made.
    pub engaged: bool,
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

    /// Carry the open decisions through a live board rebuild, which hands
    /// out new unit and city ids. `unit` and `city` map an old id to its new
    /// one. A decision whose owner or city has no new id is dropped: the next
    /// reading reopens whatever the controller still holds.
    pub(super) fn remap_ids(
        &mut self,
        unit: impl Fn(u32) -> Option<u32>,
        city: impl Fn(u32) -> Option<u32>,
    ) {
        for ((kind, owner), mut c) in std::mem::take(&mut self.open) {
            let owner = match owner {
                Owner::Unit(uid) => match unit(uid) {
                    Some(uid) => Owner::Unit(uid),
                    None => continue,
                },
                Owner::Empire => Owner::Empire,
            };
            if let Target::City(cid) = c.target {
                match city(cid) {
                    Some(cid) => c.target = Target::City(cid),
                    None => continue,
                }
            }
            c.owner = owner;
            self.open.insert((kind, owner), c);
        }
        self.cities_seen = self
            .cities_seen
            .iter()
            .filter_map(|cid| city(*cid))
            .collect();
    }

    /// `stall-rebases-on-new-walls`: the open capture's best reading starts
    /// again at `reading`, its stall streak cleared.
    pub(super) fn rebase_capture(&mut self, turn: u32, reading: i32) {
        if let Some(c) = self.open.get_mut(&(Kind::Capture, Owner::Empire)) {
            c.best = reading;
            c.best_turn = turn;
            c.stalled_streak = 0;
        }
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
        let new_turn = turn > c.observed_turn;
        if new_turn {
            c.observed_turn = turn;
            census.open_turns += 1;
            c.turns_open += 1;
        }
        // A fresh observation can reveal progress in the same turn. Update
        // the reading and reset streaks, without charging another turn.
        if reading < c.best {
            c.best = reading;
            c.best_turn = turn;
            c.stalled_streak = 0;
        }
        if reading < c.initial {
            c.engaged = true;
        }
        match acted {
            Some(false) => {
                if new_turn {
                    c.forgotten_turns += 1;
                    c.forgotten_streak += 1;
                    census.forgotten_turns += 1;
                    *self.forgotten_why.entry((key.0, why)).or_default() += 1;
                }
                c.stalled_streak = 0;
            }
            Some(true) if turn.saturating_sub(c.best_turn) >= STALL_TURNS => {
                c.forgotten_streak = 0;
                if new_turn {
                    c.stalled_turns += 1;
                    c.stalled_streak += 1;
                    census.stalled_turns += 1;
                }
            }
            Some(true) => c.forgotten_streak = 0,
            None => {}
        }
        if new_turn && turn > c.eta {
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
        self.end_open(key, "retired", turn)
    }

    /// `commitment-owner-acts`: a commitment the gene released because its
    /// next step was illegal or its owner stood inside a hostile's reach.
    /// Ends as `released`, counted with the abandoned.
    pub(super) fn release(&mut self, key: (Kind, Owner), turn: u32) -> bool {
        self.end_open(key, "released", turn)
    }

    fn end_open(&mut self, key: (Kind, Owner), how: &'static str, turn: u32) -> bool {
        match self.open.remove(&key) {
            Some(c) => {
                self.end(c, how, turn);
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
                        observed_turn: turn,
                        last_pos: Some(unit.pos),
                        improvement_then: g.map.get(*site).and_then(|tile| tile.improvement),
                        retargets: retargets + 1,
                        turns_open: 0,
                        forgotten_turns: 0,
                        forgotten_streak: 0,
                        stalled_streak: 0,
                        engaged: false,
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
                    observed_turn: turn,
                    last_pos: Some(unit.pos),
                    improvement_then: g.map.get(site).and_then(|tile| tile.improvement),
                    retargets: 0,
                    turns_open: 0,
                    forgotten_turns: 0,
                    forgotten_streak: 0,
                    stalled_streak: 0,
                    engaged: false,
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
                    } else {
                        // And once there, did the siege ever push the city
                        // below where it stood when the decision was made?
                        match (c.forgotten_turns == 0 && c.turns_open > 0, c.engaged) {
                            (true, true) => "ended, always present, engaged",
                            (true, false) => "ended, always present, never scratched it",
                            (false, true) => "ended, went then left, engaged",
                            (false, false) => "ended, went then left, never scratched it",
                        }
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
            let acted = if w.waiting.is_some() {
                None
            } else {
                w.declared.then_some(w.present)
            };
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
                observed_turn: turn,
                last_pos: None,
                improvement_then: None,
                retargets: 0,
                turns_open: 0,
                forgotten_turns: 0,
                forgotten_streak: 0,
                stalled_streak: 0,
                engaged: false,
                stalled_turns: 0,
            });
        }
    }

    /// Whether `key` is one of the counters [`Self::export`] writes, as
    /// opposed to one a ledger-acting gene writes beside them.
    pub fn exports(&self, key: &str) -> bool {
        let Some(rest) = key.strip_prefix("commit:") else {
            return false;
        };
        let field = rest.rsplit(':').next().unwrap_or(rest);
        EXPORTED_FIELDS.contains(&field)
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
            debug_assert_eq!(EXPORTED_FIELDS.len(), 11);
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

/// What `commitment-owner-acts` did for one owner the unit pass left
/// standing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum OwnerAct {
    /// The completing action was applied: a city founded, an improvement laid.
    Completed,
    /// One safe route step toward the target.
    Stepped(Pos),
    /// Held where it stands, for the named reason.
    Held(&'static str),
    /// The decision was released — target dropped, site parked — for the
    /// named reason.
    Released(&'static str),
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
    /// `siege-needs-a-breaker`: the train holds outside the city's reach
    /// while a breaker comes or while it gathers on its staging ring
    /// (`AdvancedAi::capture_waits_on_the_train`), and why; the turn is
    /// neither forgotten nor stalled.
    waiting: Option<&'static str>,
}

impl AdvancedAi {
    /// `stall-rebases-on-new-walls`: when the capture objective's wall pool
    /// grows, its best reading starts again from the walled city, so walls
    /// knocked down count as the new lows they are. The reading is hit points
    /// plus walls; a city that builds walls mid-siege jumps above the best
    /// it showed unwalled, and nothing short of breaching past that old best
    /// counted as progress. Live King civvis-20261005T021048Z (game 93) had
    /// Kwadukuza at 184 unwalled by turn 132; it built walls, the army took
    /// them from 64 to 28 of 100 over turns 136-140, and the siege was stood
    /// down at 140 as "not pushed to a new low", which opened "the war has
    /// stalled" peace at 145.
    pub(super) fn rebase_capture_on_new_walls(&mut self, g: &Game, cid: u32, reading: i32) {
        if !self.stall_rebases_on_new_walls {
            return;
        }
        let Some(city) = g.cities.get(&cid) else {
            return;
        };
        let walls = g.city_max_wall_hp(city);
        let raised = self
            .capture_walls_seen
            .is_some_and(|(pos, seen)| pos == city.pos && walls > seen);
        let open = self
            .commitments
            .open_for(Kind::Capture, Owner::Empire)
            .is_some_and(|c| c.target == Target::City(cid));
        if raised && open {
            self.commitments.rebase_capture(g.turn, reading);
            think!(self.journal(), Military, Detail,
                   "The siege of {} starts its reading again from the new walls", city.name;
                   "the wall pool grew to {walls}; walls knocked down from here are progress";
                   city.pos);
        }
        self.capture_walls_seen = Some((city.pos, walls));
    }

    /// `stall-waits-for-the-breach`: when the siege on the capture objective
    /// steps forward — Stage to Invest, Invest to Reduce, Reduce to Take —
    /// the open capture's best reading starts again from where the city
    /// stands, so the walls and hit points the assault knocks down from here
    /// are new lows. A step back and the next step forward do not restart it
    /// again inside `STALL_TURNS + CAPTURE_STALL_TURNS` turns, so a train
    /// that steps in and out of Invest is still stood down if it never
    /// dents the city. Live King civvis-20261005T181436Z (game 150): the
    /// best reading for Karkar dated from turn 71, before its walls; the
    /// guns took the walls from 100 to 87 at turn 89, the city had healed
    /// to 200, and the siege was stood down at 92 on the turn it stepped
    /// into Invest reading "damage ready true".
    pub(super) fn rebase_capture_on_a_forward_step(&mut self, g: &Game, cid: u32, reading: i32) {
        use super::siege_train::SiegeStage;
        if !self.stall_waits_for_the_breach {
            return;
        }
        let Some(city) = g.cities.get(&cid) else {
            return;
        };
        let Some(siege) = self
            .sieges
            .get(&cid)
            .filter(|siege| g.turn.saturating_sub(siege.assessed) <= 1)
        else {
            return;
        };
        let rank = match siege.stage {
            SiegeStage::Stage => 0,
            SiegeStage::Invest => 1,
            SiegeStage::Reduce => 2,
            SiegeStage::Take => 3,
            SiegeStage::Hold => 4,
        };
        let (seen, last) = match self.stall_stage_seen {
            Some((pos, seen, last)) if pos == city.pos => (Some(seen), last),
            _ => (None, None),
        };
        let forward = seen.is_some_and(|seen| rank > seen) && (1..=3).contains(&rank);
        let spaced =
            last.is_none_or(|at| g.turn.saturating_sub(at) >= STALL_TURNS + CAPTURE_STALL_TURNS);
        let open = self
            .commitments
            .open_for(Kind::Capture, Owner::Empire)
            .is_some_and(|c| c.target == Target::City(cid));
        let mut rebased = last;
        if forward && spaced && open {
            self.commitments.rebase_capture(g.turn, reading);
            rebased = Some(g.turn);
            think!(self.journal(), Military, Detail,
                   "The siege of {} starts its reading again as it steps forward", city.name;
                   "the train stepped into {}; walls and hit points knocked down from here are progress",
                   siege.stage.as_str(); city.pos);
        }
        self.stall_stage_seen = Some((city.pos, rank, rebased));
    }

    /// `stall-waits-for-the-breach`: why the capture of `cid` waits on a
    /// train still gathering around the breaker it waited for, if it does:
    /// its record in Stage, assessed this turn or the last, a breaker in the
    /// train (no `siege_breaker_waits` entry), a land soldier of ours within
    /// `STAGING_FAR`, and the last turn the capture held for that breaker
    /// within `BREAKER_WAIT_TURNS` standard turns. `siege_mustering` counts
    /// its window from the turn the train entered Stage, which the breaker
    /// wait had already spent. See [`AdvancedAi::stall_waits_for_the_breach`].
    fn muster_after_the_breaker(&self, g: &Game, pid: usize, cid: u32) -> Option<&'static str> {
        use super::siege_train::{SiegeStage, BREAKER_WAIT_TURNS, STAGING_FAR};
        if !self.stall_waits_for_the_breach {
            return None;
        }
        let (Some(siege), Some(city)) = (self.sieges.get(&cid), g.cities.get(&cid)) else {
            return None;
        };
        let (pos, waited) = self.stall_breaker_waited?;
        let gathering = pos == city.pos
            && siege.stage == SiegeStage::Stage
            && g.turn.saturating_sub(siege.assessed) <= 1
            && g.turn.saturating_sub(waited) <= g.standard_duration(BREAKER_WAIT_TURNS)
            && !self.siege_breaker_waits.contains_key(&city.pos)
            && g.units.values().any(|unit| {
                let spec = &g.rules.units[unit.kind];
                unit.owner == pid
                    && spec.class == "military"
                    && !matches!(spec.domain.as_deref(), Some("sea" | "air"))
                    && g.wdist(unit.pos, city.pos) <= STAGING_FAR
            });
        gathering.then_some("while its siege train gathers around the breaker that came")
    }

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
        // `capture-waits-on-the-march`: see `advanced/capture_march.rs`.
        let march_target = self
            .war_plan
            .as_ref()
            .map(|plan| plan.objective_city)
            .or_else(|| {
                self.plan
                    .as_ref()
                    .filter(|plan| plan.strategy == GrandStrategy::Conquest)
                    .and_then(|plan| plan.target_city)
            });
        let marching = march_target
            .filter(|cid| self.capture_marching(g, pid, *cid))
            .map(|cid| (cid, "while its army marches in"));
        let march_note = |cid: u32| {
            marching
                .filter(|(city, _)| *city == cid)
                .map(|(_, why)| why)
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
                waiting: declared
                    .then(|| {
                        self.capture_waits_on_the_train(g, pid, plan.objective_city)
                            .or_else(|| march_note(plan.objective_city))
                            .or_else(|| self.muster_after_the_breaker(g, pid, plan.objective_city))
                    })
                    .flatten(),
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
                waiting: declared
                    .then(|| {
                        self.capture_waits_on_the_train(g, pid, cid)
                            .or_else(|| march_note(cid))
                            .or_else(|| self.muster_after_the_breaker(g, pid, cid))
                    })
                    .flatten(),
            })
        });
        let war = appointed.or(conquest);
        // `stall-waits-for-the-breach`: the last turn the capture held for a
        // breaker on its way; the muster that follows counts from there.
        if self.stall_waits_for_the_breach {
            if let Some((pos, cid)) = war
                .as_ref()
                .filter(|w| w.declared)
                .and_then(|w| Some((g.cities.get(&w.city)?.pos, w.city)))
            {
                if self.waiting_for_a_breaker(g, pid, cid) {
                    self.stall_breaker_waited = Some((pos, g.turn));
                }
            }
        }
        if let Some((city, why)) = war
            .as_ref()
            .and_then(|w| Some((g.cities.get(&w.city)?, w.waiting?)))
        {
            let (name, pos) = (city.name.clone(), city.pos);
            think!(self.journal(), Military, Detail, "Holding the capture of {name} {why}";
                   "the train stands outside the city's reach on purpose, so the turn counts as neither forgotten nor stalled"; pos);
        }
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
        // `stall-rebases-on-new-walls`: see `rebase_capture_on_new_walls`.
        // `stall-waits-for-the-breach`: see `rebase_capture_on_a_forward_step`.
        if let Some(w) = war.as_ref() {
            self.rebase_capture_on_new_walls(g, w.city, w.reading);
            self.rebase_capture_on_a_forward_step(g, w.city, w.reading);
        }
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
        if self.capture_go_or_stand_down || self.capture_go_or_stand_down_2 {
            self.capture_stood_down.retain(|_, until| g.turn < *until);
            if let Some(c) = ledger.open_for(Kind::Capture, Owner::Empire) {
                let Target::City(cid) = c.target else {
                    unreachable!("a capture targets a city")
                };
                let nobody_went = c.forgotten_streak >= CAPTURE_GO_TURNS;
                // Version 2: bodies at the objective and the city not pushed
                // to a new low for CAPTURE_STALL_TURNS stalled readings — a
                // siege that is not winning is stood down the same way.
                let not_winning =
                    self.capture_go_or_stand_down_2 && c.stalled_streak >= CAPTURE_STALL_TURNS;
                if (nobody_went || not_winning) && !self.capture_stood_down.contains_key(&cid) {
                    let (turns_open, forgotten, stalled) =
                        (c.turns_open, c.forgotten_streak, c.stalled_streak);
                    self.capture_stood_down
                        .insert(cid, g.turn + CAPTURE_STAND_DOWN_TURNS);
                    let counter = if nobody_went {
                        "commit:capture:gene_stand_downs"
                    } else {
                        "commit:capture:stall_stand_downs"
                    };
                    *g.players[pid]
                        .counters
                        .entry(counter.to_string())
                        .or_insert(0) += 1;
                    if let Some(city) = g.cities.get(&cid) {
                        let (name, pos) = (city.name.clone(), city.pos);
                        if nobody_went {
                            think!(self.journal(), Military, Strategy, "Standing down the objective nobody went to";
                                   "{name} was the target for {turns_open} turns and no unit of ours came within {} hexes on the last {forgotten}",
                                   CAPTURE_PRESENCE_RADIUS; pos);
                        } else {
                            think!(self.journal(), Military, Strategy, "Standing down a siege that is not winning";
                                   "{name} was the target for {turns_open} turns; the army has been there and has not pushed it to a new low in {stalled} stalled readings"; pos);
                        }
                    }
                }
            }
        }
        // `siege-target-needs-a-road`: see `advanced/siege_road.rs`.
        self.reconcile_siege_roads(g, pid);
        let ledger = &mut self.commitments;
        ledger.cities_seen = g
            .cities
            .values()
            .filter(|city| city.owner == pid)
            .map(|city| city.id)
            .collect();
        ledger.export(g, pid);
    }

    /// `capture-go-or-stand-down` family: either version is on.
    pub(super) fn capture_stand_down_on(&self) -> bool {
        self.capture_go_or_stand_down
            || self.capture_go_or_stand_down_2
            || self.siege_target_needs_a_road
    }

    /// `capture-go-or-stand-down`: whether the gene holds this city out of
    /// the target ranking this turn.
    pub(super) fn capture_stood_down_holds(&self, g: &Game, city: u32) -> bool {
        self.capture_stand_down_on()
            && self
                .capture_stood_down
                .get(&city)
                .is_some_and(|until| g.turn < *until)
            // `siege-target-needs-a-road`: a road stand-down lifts when the
            // border that shut the road opens.
            && !self.siege_road_reopened(g, city)
    }

    /// `commitment-owner-acts`: the owners the unit pass left standing act
    /// on their decisions now.
    ///
    /// Runs once per acting turn, after `advanced_units` and before the
    /// ledger's reading, so it sees exactly what the ledger would have called
    /// forgotten: a settle or improve decision open, its owner alive with
    /// movement to spend and no order issued. Each such owner gets the
    /// decision's own next step — the city founded or the improvement laid
    /// when it stands on the target, else one route step toward it when no
    /// hostile can reach that step — or the decision is released with its
    /// reason: the step is illegal (no route, a refused order, a site that
    /// cannot be founded, a tile nothing fits), or the owner itself stands
    /// inside a hostile's reach, where the civilian-safety flee keeps
    /// priority and a decision must not pull the unit deeper in. A released
    /// site is parked for the hysteresis window, the way `commitment-patience`
    /// parks a retired one, so the next pick is a different one. The holds it
    /// leaves standing are the step that is itself inside a hostile's reach or
    /// over the settler step-risk limit: a passing raider is a wait, not a new
    /// decision, and `commitment-patience` prices how long. With
    /// `route-block-is-a-wait` on, a step that is merely not legal this turn
    /// joins them, bounded by the same `COMMITMENT_PATIENCE` — see the field
    /// doc for the live measurement that says a route block is transient too.
    ///
    /// Exact no-op while the gene is off.
    pub(super) fn commitment_owners_act(&mut self, g: &mut Game, pid: usize) {
        if !self.commitment_owner_acts {
            return;
        }
        let idle = |g: &Game, uid: u32| {
            g.units
                .get(&uid)
                .is_some_and(|unit| unit.owner == pid && !unit.acted && unit.moves_left > 0.0)
        };
        let mut owners: Vec<(Kind, u32, Pos)> = Vec::new();
        for (&uid, &site) in &self.settler_targets {
            // A settler waiting for the guard it summoned holds on purpose.
            if idle(g, uid) && !self.guard_wait.contains_key(&uid) {
                owners.push((Kind::Settle, uid, site));
            }
        }
        for (&uid, &site) in &self.builder_targets {
            if idle(g, uid) {
                owners.push((Kind::Improve, uid, site));
            }
        }
        for (kind, uid, site) in owners {
            let streak = self
                .commitments
                .open_for(kind, Owner::Unit(uid))
                .filter(|c| c.target == Target::Tile(site))
                .map_or(0, |c| c.forgotten_streak);
            let unit_kind = g.units[&uid].kind;
            let act = self.commitment_owner_act(g, pid, kind, uid, site, streak);
            let field = match act {
                OwnerAct::Completed => "owner_completed",
                OwnerAct::Stepped(_) => "owner_stepped",
                OwnerAct::Held(_) => "owner_held",
                OwnerAct::Released(_) => "owner_released",
            };
            *g.players[pid]
                .counters
                .entry(format!("commit:{}:{field}", kind.as_str()))
                .or_insert(0) += 1;
            let what = plain(&unit_kind);
            match act {
                OwnerAct::Completed => {
                    think!(self.journal(), Expansion, Detail, "{what} completes the decision the unit pass left standing";
                           "{uid} stood on {site:?} with movement and no order ({streak} forgotten turns before this one); the {} is done now",
                           kind.as_str(); site);
                }
                OwnerAct::Stepped(next) => {
                    think!(self.journal(), Expansion, Detail, "{what} acts on the decision the unit pass left standing";
                           "{uid} had movement and no order ({streak} forgotten turns before this one); it steps to {next:?}, {} hexes from {site:?}",
                           g.wdist(next, site); next);
                }
                OwnerAct::Held(why) => {
                    think!(self.journal(), Expansion, Detail, "{what} holds its decision for a turn";
                           "{uid} keeps {site:?} but does not step: {why}"; site);
                }
                OwnerAct::Released(why) => {
                    think!(self.journal(), Expansion, Detail, "{what} releases the decision it was not acting on";
                           "{uid} drops {site:?} ({streak} forgotten turns before this one): {why}; the {} is set aside for {} standard turns",
                           kind.as_str(), super::SETTLER_TARGET_HYSTERESIS_TURNS; site);
                }
            }
        }
    }

    /// One owner's act. The unit is alive, ours, unacted and has movement.
    fn commitment_owner_act(
        &mut self,
        g: &mut Game,
        pid: usize,
        kind: Kind,
        uid: u32,
        site: Pos,
        streak: u32,
    ) -> OwnerAct {
        let current = g.units[&uid].pos;
        // A linked civilian has no move of its own; the formation carrying
        // it is what did not move.
        if g.units[&uid].linked_to.is_some() {
            return OwnerAct::Held("it is linked to an escort formation that did not move");
        }
        let reach = self.barbarian_reach(g, pid, current, civilian_safety::REACH_SCAN_RADIUS);
        // The flee keeps priority. A civilian standing inside a hostile's
        // reach had the flee's say in the unit pass (or holds because no
        // reachable tile is better); the decision it carries is released
        // rather than pushed, so the next pick is made from safer ground.
        if !self.civilian_safe_at(g, pid, uid, current, &reach) {
            self.release_commitment(g, pid, kind, uid, site);
            return OwnerAct::Released(
                "it stands inside a hostile's reach, where the civilian-safety flee owns its turn",
            );
        }
        if current == site {
            return match kind {
                Kind::Settle => self.commitment_owner_founds(g, pid, uid, site),
                Kind::Improve => self.commitment_owner_improves(g, pid, uid, site),
                Kind::Capture => OwnerAct::Held("a capture has no unit owner"),
            };
        }
        // `route-block-is-a-wait`: a step that is not legal THIS TURN is a
        // wait, not a new decision. `route_step` returns `None` for a zone of
        // control exactly as it does for no route at all (the predicate is
        // private to `game`), and `can_move` refuses a tile a passing unit
        // will vacate — so the unbounded release below discards sites over
        // blockers that clear on their own. Measured over 53 live runs: 502
        // of 547 releases are this one turn, and 18% of the settle drops sit
        // within two tiles of the site. Hold until the block outlasts
        // `COMMITMENT_PATIENCE` consecutive forgotten turns, which
        // `reconcile_units` counts whether or not `commitment_patience` is on.
        let route_step = g
            .route_step(uid, site, 0)
            .filter(|next| g.can_move(uid, *next));
        let Some(next) = route_step else {
            if self.route_block_is_a_wait && streak < COMMITMENT_PATIENCE {
                return OwnerAct::Held("no legal route step toward it this turn");
            }
            self.release_commitment(g, pid, kind, uid, site);
            return OwnerAct::Released("no legal route step toward it this turn");
        };
        if !self.civilian_safe_at(g, pid, uid, next, &reach) {
            return OwnerAct::Held("the next step is inside a hostile's reach");
        }
        if kind == Kind::Settle && self.settlement_safety {
            let visible = self.battlefront_visibility(g, pid);
            if self.settlement_tile_risk(g, pid, Some(uid), next, &visible)
                > super::SETTLER_STEP_RISK_LIMIT
            {
                return OwnerAct::Held("the next step is over the settler step-risk limit");
            }
        }
        if self.base.path_move(g, pid, uid, next) {
            OwnerAct::Stepped(next)
        } else if self.route_block_is_a_wait && streak < COMMITMENT_PATIENCE {
            // Same patience as the no-step case above: the mover refused this
            // turn's step, which the next turn may well allow.
            OwnerAct::Held("the route step toward it was refused")
        } else {
            self.release_commitment(g, pid, kind, uid, site);
            OwnerAct::Released("the route step toward it was refused")
        }
    }

    /// A settler standing on its site with movement and no order founds the
    /// city, under the same safety and loyalty bars the unit pass applies.
    fn commitment_owner_founds(
        &mut self,
        g: &mut Game,
        pid: usize,
        uid: u32,
        site: Pos,
    ) -> OwnerAct {
        if !g.can_found_city(uid) {
            self.release_commitment(g, pid, Kind::Settle, uid, site);
            return OwnerAct::Released(
                "it stands on the site and the city cannot be founded there",
            );
        }
        if self.settlement_safety {
            let visible = self.battlefront_visibility(g, pid);
            if self.settlement_tile_risk(g, pid, Some(uid), site, &visible)
                > super::SETTLER_STEP_RISK_LIMIT
            {
                return OwnerAct::Held("the site is over the settler step-risk limit this turn");
            }
        }
        let science_targeted = self.active_victory_target(g) == Some(super::VictoryTarget::Science);
        let verdict = if self.base.loyalty_rate_alarm || science_targeted {
            self.settle_site_loyalty_verdict(g, pid, site)
        } else {
            self.settle_site_frontier_loyalty_verdict(g, pid, site)
        };
        if verdict.is_some() {
            self.release_commitment(g, pid, Kind::Settle, uid, site);
            return OwnerAct::Released("the loyalty forecast refuses the site");
        }
        let worth = self.settle_value(g, pid, site);
        self.settler_targets.remove(&uid);
        self.settler_stalls.remove(&uid);
        self.settler_closest.remove(&uid);
        let yielded = self.hidden_border_yield_before_founding(g, pid, site);
        let fogged = self.fog_yield_before_founding(g, pid, site);
        if g.apply(pid, &Action::FoundCity { unit: uid }).is_ok() {
            self.note_hidden_border_founding(site, yielded);
            self.note_fog_founding(site, fogged);
            think!(self.journal(), Expansion, Decision, "Founding a city at {site:?} for a decision the unit pass left standing";
                   "the site is worth {worth:.1}"; site);
            OwnerAct::Completed
        } else {
            self.release_commitment(g, pid, Kind::Settle, uid, site);
            OwnerAct::Released("the engine refused the founding")
        }
    }

    /// A Builder standing on its tile with movement and no order lays the
    /// best improvement that fits it, or releases a tile nothing fits.
    fn commitment_owner_improves(
        &mut self,
        g: &mut Game,
        pid: usize,
        uid: u32,
        site: Pos,
    ) -> OwnerAct {
        let Some(strategy) = self.plan.as_ref().map(|plan| plan.strategy) else {
            return OwnerAct::Held("no strategic plan prices the tile this turn");
        };
        let Some(improvement) = self
            .worthwhile_improvements(g, pid, site, strategy)
            .into_iter()
            .next()
        else {
            self.release_commitment(g, pid, Kind::Improve, uid, site);
            return OwnerAct::Released("it stands on the tile and no improvement fits it");
        };
        self.builder_targets.remove(&uid);
        if g.apply(
            pid,
            &Action::Improve {
                unit: uid,
                improvement,
            },
        )
        .is_ok()
        {
            OwnerAct::Completed
        } else {
            self.release_commitment(g, pid, Kind::Improve, uid, site);
            OwnerAct::Released("the engine refused the improvement")
        }
    }

    /// Drop the target, park the site for the hysteresis window, and end the
    /// open commitment as `released`.
    fn release_commitment(&mut self, g: &mut Game, pid: usize, kind: Kind, uid: u32, site: Pos) {
        let until = g.turn + g.standard_duration(super::SETTLER_TARGET_HYSTERESIS_TURNS);
        match kind {
            Kind::Settle => {
                self.settler_targets.remove(&uid);
                self.settler_stalls.remove(&uid);
                self.settler_closest.remove(&uid);
                self.settler_dead_sites
                    .entry(uid)
                    .or_default()
                    .insert(site, until);
            }
            Kind::Improve => {
                self.builder_targets.remove(&uid);
                self.builder_avoid.insert(uid, (site, until));
            }
            Kind::Capture => return,
        }
        let key = (kind, Owner::Unit(uid));
        if self
            .commitments
            .open_for(kind, Owner::Unit(uid))
            .is_some_and(|c| c.target == Target::Tile(site))
        {
            self.commitments.release(key, g.turn);
        }
        *g.players[pid]
            .counters
            .entry(format!("commit:{}:released", kind.as_str()))
            .or_insert(0) += 1;
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
mod tests;

#[cfg(test)]
mod replan_tests;
