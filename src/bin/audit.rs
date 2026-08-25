//! Plays full games and reports what looked wrong while they ran.
//!
//! `soak` answers "did the game finish"; this answers "was the game any
//! good". It walks every turn of a live game and records both hard invariant
//! breaks (state the rules should never produce) and soft symptoms - idle
//! units, cities producing nothing, treasuries nobody spends - which are the
//! shape most engine and AI defects actually take from the outside.
//!
//! Usage: audit [--games N] [--start-seed N] [--players N] [--turns N]
//!              [--width N] [--height N] [--city-states N] [--speed ID]
//!              [--genome deployment|stock] [--quiet]
//!
//! ## ⚠⚠ WHICH AGENT THIS AUDITS, AND WHY THE DEFAULT CHANGED
//!
//! `AdvancedAi::fleet` is `AdvancedAi::new()` per seat: the STOCK controller,
//! no repair bundle, no ledger. The native deployment genome is
//! `enable_engine_repairs()` — the repair universe, then the ledger's
//! defaults. Those are different agents, and this binary used the first while
//! its output read as though it described the second.
//!
//! What that cost, measured on 2026-08-25 at this binary's own profile, seed
//! 21000000, turn 165, summed across the board:
//!
//! | configuration | live trade routes | `solvency_first_trade_slot` |
//! |---|---:|---|
//! | `fleet()` — what this binary used | **2** | false |
//! | `enable_engine_repairs()` — deployment | **37** | true |
//!
//! `solvency-first-trade-slot` is **rank 1 of `HEURISTIC_GENE_RANKING.md`** at
//! +8.07% (22.85% against 14.78%, P(>0) 100.0%) and has been `on` in the
//! deployment default throughout. A census of "the empire uses 3% of its trade
//! capacity" was therefore a census of an agent nobody runs.
//!
//! `--genome deployment` is now the default and every report names the genome
//! it audited. `--genome stock` keeps the old behaviour for anyone who wants
//! the bare controller, which is a legitimate question — just not the one the
//! unqualified word "audit" implies.
//!
//! ## What the lane census measured (2026-08-19, 16 games)
//!
//! At the live ladder's profile — `--games 16 --start-seed 21000000
//! --players 6 --turns 250 --speed online`, which defaults to 74x46 with 9
//! city-states — the engine finished: **religious 9, score 5, science 1,
//! culture 1, diplomatic 0**.
//!
//! The planner-turn shares behind that:
//!
//! | lane | games below a twentieth of the board's planning |
//! |---|---:|
//! | diplomacy | **16 of 16** (0% in every game) |
//! | culture | 11 of 16 |
//! | science | 2 of 16 |
//!
//! `docs/EVAL_STATUS.md` counts how the live ladder actually loses: of 83
//! attempts ended by a rival's victory, **diplomatic 47 and culture 27** —
//! 74 of the 83. The two lanes that take three quarters of our losses are
//! the two this board contests least, and diplomacy it does not contest at
//! all: zero planner-turns in all sixteen games, with the diplomatic victory
//! condition enabled the whole time.
//!
//! This is a measurement, not a diagnosis. It does not say why the Diplomacy
//! grand strategy is never adopted, and the fixed-priority fallback in
//! `AdvancedAi::rival_victory_pressure` is *not* the explanation — stock
//! agents run with `victory_planning` on, so that branch never executes.
//!
//! The tourism half: the board's tourism leader was granted Open Borders by
//! 15 host-civilizations across the 16 games. In the one native culture
//! victory, 2 of 5 hosts had granted it and 5 of 5 carried the winner's
//! Trade Routes — so the +25% a host chooses to hand over is a real but
//! secondary contributor, and the Trade Route half is the visitor's own move.
//! A denial treatment aimed at Open Borders should be priced against that,
//! not against the assumption that consent is what pays for the lane.
use std::collections::{BTreeMap, HashSet};

use civvis::ai::{AdvancedAi, Ai};
use civvis::game::{default_speed, Action, Game, GameOptions, WarRecord};
use civvis::rules::Rules;
use civvis::setup::MapSize;

fn number(args: &[String], flag: &str, default: i64) -> i64 {
    args.iter()
        .position(|arg| arg == flag)
        .and_then(|index| args.get(index + 1))
        .and_then(|value| value.parse().ok())
        .unwrap_or(default)
}

fn text(args: &[String], flag: &str, default: &str) -> String {
    args.iter()
        .position(|arg| arg == flag)
        .and_then(|index| args.get(index + 1))
        .cloned()
        .unwrap_or_else(|| default.to_string())
}

/// How long a unit or city may sit doing nothing before it is worth reporting.
const IDLE_TURNS: u32 = 25;
const WAR_MIN_TURNS: u32 = 10;

/// A unit walking in a circle looks busy from the outside and is invisible to
/// the idle checks above, which only ever notice a unit that stops. Ten turns
/// of movement confined to three tiles, with nothing about the unit changing
/// in all that time, is a livelock: no errand a real unit runs — a Builder
/// working two tiles beside a city, a garrison shuffling around a wall — takes
/// that long without spending a charge, taking a hit, or earning experience.
const LIVELOCK_TURNS: u32 = 10;
const LIVELOCK_FOOTPRINT: usize = 3;

/// The minimum applies to a negotiated settlement, not to eliminating the
/// opposing civilization. A quick conquest is a decisive war, not diplomacy
/// undoing a declaration before its commitment has run.
fn negotiated_war_ended_early(war: &WarRecord, minimum_turns: u32) -> bool {
    let Some(ended) = war.ended else {
        return false;
    };
    war.highlights
        .last()
        .is_some_and(|highlight| highlight.kind == "peace")
        && ended.saturating_sub(war.started) < minimum_turns
}

/// Only negotiated peace creates the treaty whose cooldown this audit checks.
/// Emergency coalitions can compel two members to stop fighting, and conquest
/// can close a ledger record too; neither is a peace agreement between the
/// pair, so a later war must not be reported as violating a treaty that never
/// existed.
fn redeclared_inside_peace_treaty(
    previous: &WarRecord,
    next: &WarRecord,
    treaty_turns: u32,
) -> bool {
    let previous_pair = (
        previous.aggressor.min(previous.defender),
        previous.aggressor.max(previous.defender),
    );
    let next_pair = (
        next.aggressor.min(next.defender),
        next.aggressor.max(next.defender),
    );
    let Some(ended) = previous.ended else {
        return false;
    };
    previous_pair == next_pair
        && previous
            .highlights
            .last()
            .is_some_and(|highlight| highlight.kind == "peace")
        && next.started.saturating_sub(ended) < treaty_turns
}

fn rapid_recapture_window(war: &WarRecord) -> Option<(String, u32, u32)> {
    let mut captures: BTreeMap<String, Vec<u32>> = BTreeMap::new();
    for highlight in &war.highlights {
        if matches!(highlight.kind.as_str(), "city_captured" | "capital_captured") {
            if let Some(city) = &highlight.city {
                captures.entry(city.clone()).or_default().push(highlight.turn);
            }
        }
    }
    captures.into_iter().find_map(|(city, turns)| {
        turns
            .windows(3)
            .find(|window| window[2].saturating_sub(window[0]) <= WAR_MIN_TURNS)
            .map(|window| (city, window[0], window[2]))
    })
}

fn treasury_looks_hoarded(is_minor: bool, gold: f64, gold_per_turn: f64) -> bool {
    let income_buffer = if is_minor {
        1_000.0
    } else {
        (gold_per_turn.max(0.0) * 20.0).max(1_000.0)
    };
    gold > income_buffer
}

#[derive(Default)]
struct Findings {
    /// Rules the engine broke, keyed by a short signature so one recurring
    /// fault reports as one line with a count rather than thousands.
    violations: BTreeMap<String, (usize, String)>,
    /// Symptoms that are legal but suggest something is not working.
    symptoms: BTreeMap<String, (usize, String)>,
}

impl Findings {
    fn violation(&mut self, key: impl Into<String>, detail: impl Into<String>) {
        let entry = self
            .violations
            .entry(key.into())
            .or_insert_with(|| (0, detail.into()));
        entry.0 += 1;
    }

    fn symptom(&mut self, key: impl Into<String>, detail: impl Into<String>) {
        let entry = self
            .symptoms
            .entry(key.into())
            .or_insert_with(|| (0, detail.into()));
        entry.0 += 1;
    }
}

/// State that only means something across turns, so it cannot be judged from
/// a single snapshot.
#[derive(Default)]
struct History {
    unit_still_since: BTreeMap<u32, (u32, civvis::Pos)>,
    city_idle_since: BTreeMap<u32, u32>,
    trader_ready_since: BTreeMap<u32, u32>,
    reported_unit: BTreeMap<u32, bool>,
    reported_city: BTreeMap<u32, bool>,
    tracks: BTreeMap<u32, Track>,
    /// Per major unit, for its whole life: its kind, whether it has ever
    /// changed tile, and whether its work mark has ever moved.
    ///
    /// ★★★★★ THIS IS THE QUESTION THE IDLE TABLE CANNOT ASK. A share tells you
    /// how much of a kind's time is spent standing still; it cannot separate a
    /// fleet that works hard and rests from a fleet half of which was never
    /// usable. The Galley was the second: 53 hulls built and **20 that never
    /// moved once**, because a lakeside city counts as coastal and its hull can
    /// never leave the lake (#1989, promoted #1997). The idle share said 54.3%
    /// and did not say that.
    lives: BTreeMap<u32, UnitLife>,
}

#[derive(Clone)]
struct UnitLife {
    kind: String,
    turns: u64,
    moved: bool,
    worked: bool,
    last: civvis::Pos,
    last_mark: WorkMark,
}

/// Everything about a unit that changes when it accomplishes something:
/// charges spent improving or spreading, experience from a fight, damage taken
/// or healed, a promotion chosen, a concert played. A unit whose whole mark is
/// unchanged has, whatever else it did, achieved nothing.
type WorkMark = (i32, i64, i32, usize, i64);

fn work_mark(g: &Game, id: u32) -> WorkMark {
    let unit = &g.units[&id];
    (
        unit.charges,
        unit.xp,
        unit.hp,
        unit.promotions.len(),
        unit.album_sales,
    )
}

/// One unmoving-or-circling episode: it starts whenever the unit's work mark
/// changes or it leaves the footprint, so a unit that is genuinely travelling
/// or genuinely working never accumulates one.
struct Track {
    since: u32,
    last: civvis::Pos,
    tiles: Vec<civvis::Pos>,
    /// Turns within this episode on which the unit changed tile.
    moves: u32,
    work: WorkMark,
    reported: bool,
}

/// Unit-turns by what the unit was doing with them, so a fix that only
/// converts one failure into another is visible rather than flattering.
#[derive(Default, Clone, Copy)]
struct Motion {
    unit_turns: u64,
    /// Moved, inside a footprint of at most three tiles, achieving nothing.
    livelock: u64,
    /// Stood still in the open, unfortified, achieving nothing.
    idle_field: u64,
    /// Of those, the ones that COULD have fortified: unembarked land military.
    ///
    /// The split is worth keeping: a settler or a trader standing still is not
    /// squandering anything it had, while a warrior standing still is giving up
    /// **+3 combat strength per fortified turn, capped at +6**
    /// (`unit_strength`), about 30% of its base.
    ///
    /// Read the column as *description*, not as a backlog item: it identifies
    /// idle units without assuming that fortifying them is an outcome win.
    idle_could_fortify: u64,
    /// Stood still in the open, fortified. A picket is legitimate; a
    /// stampede into this column is a livelock fix that only hid the problem.
    picket: u64,
}

impl Motion {
    fn add(&mut self, other: Motion) {
        self.unit_turns += other.unit_turns;
        self.livelock += other.livelock;
        self.idle_field += other.idle_field;
        self.idle_could_fortify += other.idle_could_fortify;
        self.picket += other.picket;
    }

    fn line(&self) -> String {
        let rate = |count: u64| {
            if self.unit_turns == 0 {
                0.0
            } else {
                100.0 * count as f64 / self.unit_turns as f64
            }
        };
        format!(
            "unit-turns={} livelock={} ({:.2}%) idle-field={} ({:.2}%) \
             of-which-fortifiable={} ({:.2}%) picket={} ({:.2}%)",
            self.unit_turns,
            self.livelock,
            rate(self.livelock),
            self.idle_field,
            rate(self.idle_field),
            self.idle_could_fortify,
            rate(self.idle_could_fortify),
            self.picket,
            rate(self.picket),
        )
    }
}

/// Split motion waste by controller role. City-states are deliberately
/// restricted to a small defense radius and barbarians have no empire to
/// develop, so mixing either into the major-civilization rate can make a
/// healthy rated AI look idle (or hide a regression behind a changing minor
/// population).
#[derive(Default)]
struct MotionBreakdown {
    major: Motion,
    minor: Motion,
    barbarian: Motion,
    /// Major unit-turns and idle unit-turns, per unit kind.
    ///
    /// The role split above says how much of the game is spent standing still;
    /// it does not say *what* is standing still, and the two answers want
    /// different fixes. A galley with no objective, a builder with no charge
    /// left to spend, and a warrior that declined every trade are three
    /// separate defects that the one 20% figure reports as a single number.
    major_by_kind: BTreeMap<String, (u64, u64)>,
    /// Per major unit kind: how many were ever seen, how many never acted at
    /// all, and how many of those lived long enough for that to be a decision
    /// rather than a birth. See `History::lives`.
    major_lives: BTreeMap<String, (u64, u64, u64)>,
}

fn controller_role(g: &Game, owner: usize) -> &'static str {
    let player = &g.players[owner];
    if player.is_barbarian {
        "barbarian"
    } else if player.is_minor {
        "city-state"
    } else {
        "major"
    }
}

impl MotionBreakdown {
    fn for_owner(&mut self, g: &Game, owner: usize) -> &mut Motion {
        match controller_role(g, owner) {
            "major" => &mut self.major,
            "city-state" => &mut self.minor,
            "barbarian" => &mut self.barbarian,
            _ => unreachable!(),
        }
    }

    fn add(&mut self, other: &MotionBreakdown) {
        self.major.add(other.major);
        self.minor.add(other.minor);
        self.barbarian.add(other.barbarian);
        for (kind, (turns, idle)) in &other.major_by_kind {
            let entry = self.major_by_kind.entry(kind.clone()).or_insert((0, 0));
            entry.0 += turns;
            entry.1 += idle;
        }
        for (kind, (built, never, never_long)) in &other.major_lives {
            let entry = self.major_lives.entry(kind.clone()).or_insert((0, 0, 0));
            entry.0 += built;
            entry.1 += never;
            entry.2 += never_long;
        }
    }

    fn total(&self) -> Motion {
        let mut total = self.major;
        total.add(self.minor);
        total.add(self.barbarian);
        total
    }

    fn print(&self, indent: &str) {
        println!("{indent}motion all        {}", self.total().line());
        println!("{indent}       major      {}", self.major.line());
        println!("{indent}       city-state {}", self.minor.line());
        println!("{indent}       barbarian  {}", self.barbarian.line());
        self.print_major_kinds(indent);
        self.print_major_lives(indent);
    }

    /// Units that lived and never did one thing.
    ///
    /// The idle table above is a share of time; this is a count of units. A
    /// kind can idle 50% of its turns because it works in bursts, or because
    /// half of the ones built were never usable, and only this line tells them
    /// apart. It is printed narrow on purpose: a kind with no never-actors is
    /// not news.
    fn print_major_lives(&self, indent: &str) {
        let mut rows: Vec<(&String, u64, u64, u64)> = self
            .major_lives
            .iter()
            .filter(|(_, (_, _, never_long))| *never_long > 0)
            .map(|(kind, (built, never, never_long))| (kind, *built, *never, *never_long))
            .collect();
        rows.sort_by(|left, right| {
            right
                .3
                .cmp(&left.3)
                .then(right.1.cmp(&left.1))
                .then(left.0.cmp(right.0))
        });
        if rows.is_empty() {
            return;
        }
        println!(
            "{indent}       major units that never acted (lived {NEVER_ACTED_MIN_TURNS}+ turns and \
             never moved or worked, of all built)"
        );
        for (kind, built, _never, never_long) in rows.iter().take(10) {
            println!(
                "{indent}         {kind:<24} {never_long:>4} of {built:<4}  {:>5.1}% of the ones built",
                100.0 * *never_long as f64 / (*built).max(1) as f64,
            );
        }
    }

    /// The major idle-field share, attributed to the units that spend it.
    /// Ordered by idle unit-turns, because the question a fix has to answer is
    /// where the mass is, not which kind has the worst rate on four turns.
    fn print_major_kinds(&self, indent: &str) {
        let mut rows: Vec<(&String, u64, u64)> = self
            .major_by_kind
            .iter()
            .filter(|(_, (_, idle))| *idle > 0)
            .map(|(kind, (turns, idle))| (kind, *turns, *idle))
            .collect();
        rows.sort_by(|left, right| right.2.cmp(&left.2).then(left.0.cmp(right.0)));
        let total_idle: u64 = rows.iter().map(|row| row.2).sum();
        if total_idle == 0 {
            return;
        }
        println!("{indent}       major idle-field by unit (idle unit-turns, share of major idle, own idle rate)");
        for (kind, turns, idle) in rows.iter().take(12) {
            println!(
                "{indent}         {kind:<24} {idle:>6}  {:>5.1}% of idle  {:>5.1}% of its own turns",
                100.0 * *idle as f64 / total_idle as f64,
                if *turns == 0 {
                    0.0
                } else {
                    100.0 * *idle as f64 / *turns as f64
                },
            );
        }
        if rows.len() > 12 {
            let rest: u64 = rows.iter().skip(12).map(|row| row.2).sum();
            println!(
                "{indent}         {:<24} {rest:>6}  {:>5.1}% of idle",
                format!("({} other kinds)", rows.len() - 12),
                100.0 * rest as f64 / total_idle as f64
            );
        }
    }
}

/// A unit has to have lived a while before never acting is a verdict on the
/// controller rather than on when the game ended.
const NEVER_ACTED_MIN_TURNS: u64 = 15;

/// Update one unit's episode and account for this turn of its life. Returns a
/// livelock detail exactly once per episode, when it first crosses the
/// threshold, while every later turn of the same episode still counts toward
/// the livelock share — so the symptom count reads as "how many units" and the
/// share reads as "how much of the game".
fn track_unit(g: &Game, history: &mut History, id: u32, motion: &mut Motion) -> Option<String> {
    let unit = &g.units[&id];
    let mark = work_mark(g, id);
    let track = history.tracks.entry(id).or_insert_with(|| Track {
        since: g.turn,
        last: unit.pos,
        tiles: vec![unit.pos],
        moves: 0,
        work: mark,
        reported: false,
    });
    motion.unit_turns += 1;

    let restart = |track: &mut Track| {
        track.since = g.turn;
        track.last = unit.pos;
        track.tiles = vec![unit.pos];
        track.moves = 0;
        track.work = mark;
        track.reported = false;
    };

    if track.work != mark {
        restart(track);
        return None;
    }
    if track.last != unit.pos {
        track.moves += 1;
        track.last = unit.pos;
        if !track.tiles.contains(&unit.pos) {
            track.tiles.push(unit.pos);
        }
        // A unit that has reached a fourth tile is going somewhere. Judge it
        // afresh from here rather than against where it set out.
        if track.tiles.len() > LIVELOCK_FOOTPRINT {
            restart(track);
            return None;
        }
    } else if unit.fortified {
        motion.picket += 1;
    } else if g
        .city_at(unit.pos)
        .is_none_or(|city| g.cities[&city].owner != unit.owner)
    {
        motion.idle_field += 1;
        // `unit_can_fortify` is private to the engine, so mirror its three
        // conditions rather than widen the API for a diagnostic.
        let spec = &g.rules.units[unit.kind];
        if spec.class == "military"
            && spec.domain.as_deref() != Some("sea")
            && !g.is_embarked(unit)
        {
            motion.idle_could_fortify += 1;
        }
    }

    let elapsed = g.turn.saturating_sub(track.since);
    // Only a unit that keeps moving is circling; one that has stopped is the
    // separate stall the checks above already cover.
    let circling = track.tiles.len() >= 2 && track.moves * 2 >= elapsed;
    if elapsed < LIVELOCK_TURNS || !circling {
        return None;
    }
    motion.livelock += 1;
    if track.reported {
        return None;
    }
    track.reported = true;
    Some(format!(
        "unit {id} ({}) of {} circled {:?} for {elapsed} turns from turn {}",
        unit.kind,
        g.players[unit.owner].civ,
        track.tiles,
        track.since,
    ))
}

fn unit_had_idle_opportunity(
    history: &mut History,
    id: u32,
    kind: &str,
    turn: u32,
    active_routes: i64,
    capacity: i64,
    route_available: bool,
) -> bool {
    if kind != "trader" {
        return true;
    }
    if active_routes >= capacity || !route_available {
        history.trader_ready_since.remove(&id);
        return false;
    }
    let ready = *history.trader_ready_since.entry(id).or_insert(turn);
    turn > ready
}

fn stalled_settler_context(g: &Game, id: u32) -> String {
    let unit = &g.units[&id];
    let pid = unit.owner;
    let legal_sites: Vec<_> = g
        .map
        .tiles
        .iter()
        .filter(|(position, tile)| {
            // Every clause `Game::can_found_city` applies, natural wonders
            // included: counting a tile the engine refuses inflates
            // `legal_sites` and, when it is the tile underfoot, is what made
            // `exhaustive_step` disagree with `reachable`.
            !g.rules.is_water(tile)
                && g.rules.is_passable(tile)
                && !g.tile_is_natural_wonder(tile)
                && !g
                    .cities
                    .values()
                    .any(|city| g.wdist(city.pos, **position) < 4)
                && tile
                    .owner_city
                    .is_none_or(|city| g.cities[&city].owner == pid)
        })
        .map(|(position, _)| *position)
        .collect();
    let reachable = legal_sites
        .iter()
        .filter(|position| **position == unit.pos || g.route_step(id, **position, 0).is_some())
        .count();
    // A Settler cannot route to the tile it is already standing on, and
    // leaving that tile in the goal set makes `route_step_to_any` short-circuit
    // on `is_goal(start)` and answer None. A Settler parked on a perfectly good
    // site then reads as one that cannot reach any site at all — the opposite
    // of the truth, and the opposite of what the next reader needs to know.
    let goals: HashSet<_> = legal_sites
        .iter()
        .copied()
        .filter(|position| *position != unit.pos)
        .collect();
    let exhaustive_step = g.route_step_to_any(id, &goals);
    format!(
        "; at {:?}, cities={}, can_found_here={}, legal_sites={}, reachable={}, \
         exhaustive_step={:?}, shipbuilding={}, linked={:?}",
        unit.pos,
        g.player_city_ids(pid).len(),
        g.can_found_city(id),
        legal_sites.len(),
        reachable,
        exhaustive_step,
        g.players[pid].techs.contains(&civvis::name!("shipbuilding")),
        unit.linked_to,
    )
}

fn stalled_trader_context(g: &Game, id: u32) -> String {
    let unit = &g.units[&id];
    let pid = unit.owner;
    let traders = g
        .units
        .values()
        .filter(|candidate| candidate.owner == pid && candidate.kind == "trader")
        .count();
    let legal_routes = g
        .legal_actions(pid)
        .into_iter()
        .filter(|action| matches!(action, Action::TradeRoute { unit, .. } if *unit == id))
        .count();
    let city = g.city_at(unit.pos).map(|city| g.cities[&city].name.clone());
    format!(
        "; at {:?}, city={city:?}, capacity={}, active_routes={}, available_traders={traders}, legal_routes={legal_routes}",
        unit.pos,
        g.trade_capacity(pid),
        g.active_routes(pid),
    )
}

fn idle_city_context(g: &Game, id: u32) -> String {
    let city = &g.cities[&id];
    let player = &g.players[city.owner];
    let producible = g.producible_items(city.owner, id);
    let districts: Vec<_> = city.districts.keys().cloned().collect();
    format!(
        "; pop={}, Gold={:.0}, GPT={:.1}, districts={districts:?}, buildings={}, producible={} {producible:?}",
        city.pop,
        player.gold,
        player.gold_per_turn,
        city.buildings.len(),
        producible.len(),
    )
}

fn city_state_district_family(g: &Game, pid: usize) -> &'static str {
    match g.cs_type(&g.players[pid].civ) {
        "scientific" => "campus",
        "cultural" => "theater_square",
        "religious" => "holy_site",
        "militaristic" => "encampment",
        "industrial" => "industrial_zone",
        _ => "commercial_hub",
    }
}

fn district_family(g: &Game, district: civvis::name::Name) -> civvis::name::Name {
    let mut current = district;
    for _ in 0..g.rules.districts.len() {
        let Some(parent) = g.rules.districts[&current].replaces else {
            break;
        };
        current = parent;
    }
    current
}

fn bounded_minor_idle(
    g: &Game,
    pid: usize,
    military: usize,
    producible: &[civvis::game::Item],
) -> bool {
    let specialty = city_state_district_family(g, pid);
    let actionable_infrastructure = producible.iter().any(|item| match item {
        civvis::game::Item::Repair { .. } | civvis::game::Item::Building { .. } => true,
        civvis::game::Item::District { district, .. } => {
            district_family(g, *district).as_str() == specialty
        }
        civvis::game::Item::Project { project } => {
            matches!(project.as_str(), "repair_outer_defenses" | "repair_encampment")
        }
        _ => false,
    });
    g.players[pid].is_minor
        && military >= 3
        && !actionable_infrastructure
}

fn audit_turn(
    g: &Game,
    history: &mut History,
    found: &mut Findings,
    motion: &mut MotionBreakdown,
) {
    history.tracks.retain(|id, _| g.units.contains_key(id));
    for (id, unit) in &g.units {
        let role = controller_role(g, unit.owner);
        // Attribute this unit-turn before the shared counters move, so the
        // per-kind ledger is the same accounting split a different way rather
        // than a second, independently drifting one.
        let before = *motion.for_owner(g, unit.owner);
        if let Some(detail) = track_unit(g, history, *id, motion.for_owner(g, unit.owner)) {
            found.symptom(
                format!(
                    "{role} {} circles without progress {LIVELOCK_TURNS}+ turns",
                    unit.kind
                ),
                detail,
            );
        }
        if role == "major" {
            // Whole-life ledger, kept beside the per-turn one. `work_mark`
            // already carries everything that changes when a unit accomplishes
            // something, so "has this unit ever done anything" is one
            // comparison rather than a second definition of work.
            let mark = work_mark(g, *id);
            let life = history.lives.entry(*id).or_insert_with(|| UnitLife {
                kind: unit.kind.to_string(),
                turns: 0,
                moved: false,
                worked: false,
                last: unit.pos,
                last_mark: mark,
            });
            life.turns += 1;
            // Self-contained on purpose: the two comparisons below are against
            // this ledger's own previous values, not against `tracks` or
            // `unit_still_since`, both of which are rewritten elsewhere in this
            // same loop and would make the answer depend on statement order.
            life.moved |= life.last != unit.pos;
            life.worked |= life.last_mark != mark;
            life.last = unit.pos;
            life.last_mark = mark;
            let after = *motion.for_owner(g, unit.owner);
            let entry = motion
                .major_by_kind
                .entry(unit.kind.to_string())
                .or_insert((0, 0));
            entry.0 += after.unit_turns - before.unit_turns;
            entry.1 += after.idle_field - before.idle_field;
        }
        if unit.hp <= 0 || unit.hp > 100 {
            found.violation(
                "unit hp out of range",
                format!("unit {id} ({}) at hp {}", unit.kind, unit.hp),
            );
        }
        if unit.moves_left < -f64::EPSILON {
            found.violation(
                "negative movement",
                format!("unit {id} ({}) at {} MP", unit.kind, unit.moves_left),
            );
        }
        if g.map.get(unit.pos).is_none() {
            found.violation(
                "unit off the map",
                format!("unit {id} ({}) at {:?}", unit.kind, unit.pos),
            );
        }
        if !g.players[unit.owner].alive {
            found.violation(
                "unit outlives its owner",
                format!("unit {id} ({}) owned by {}", unit.kind, unit.owner),
            );
        }

        // A unit that never moves is usually a pathing or target-selection
        // dead end rather than a deliberate garrison, so only flag the ones
        // that are not fortified in place.
        let route_available = unit.kind != "trader"
            || g.legal_actions(unit.owner)
                .into_iter()
                .any(|action| matches!(action, Action::TradeRoute { unit, .. } if unit == *id));
        let had_idle_opportunity = unit_had_idle_opportunity(
            history,
            *id,
            &unit.kind,
            g.turn,
            g.active_routes(unit.owner),
            g.trade_capacity(unit.owner),
            route_available,
        );
        let entry = history
            .unit_still_since
            .entry(*id)
            .or_insert((g.turn, unit.pos));
        if entry.1 != unit.pos {
            *entry = (g.turn, unit.pos);
        } else if g.turn - entry.0 >= IDLE_TURNS
            && !unit.fortified
            // Losing Merchant Republic or a temporary policy slot can leave
            // a Trader waiting behind active routes. A slot can then open in
            // player zero's `begin_turn`, immediately before this round-level
            // audit but before that AI has acted. Give every Trader one full
            // turn with capacity and a legal route before calling it idle.
            && had_idle_opportunity
            && !history.reported_unit.get(id).copied().unwrap_or(false)
        {
            history.reported_unit.insert(*id, true);
            let context = match unit.kind.as_str() {
                "settler" => stalled_settler_context(g, *id),
                "trader" => stalled_trader_context(g, *id),
                _ => String::new(),
            };
            found.symptom(
                format!("{role} {} sits still {IDLE_TURNS}+ turns", unit.kind),
                format!(
                    "unit {id} ({}) of {} unmoved since turn {}{context}",
                    unit.kind, g.players[unit.owner].civ, entry.0,
                ),
            );
        }
    }

    for (id, city) in &g.cities {
        if city.pop < 1 {
            found.violation("city below one Citizen", format!("city {id} ({})", city.name));
        }
        if !g.players[city.owner].alive {
            found.violation(
                "city outlives its owner",
                format!("city {id} ({}) owned by {}", city.name, city.owner),
            );
        }
        if city.loyalty < -f64::EPSILON || city.loyalty > 100.0 + f64::EPSILON {
            found.violation(
                "loyalty out of range",
                format!("city {id} ({}) at {} Loyalty", city.name, city.loyalty),
            );
        }
        // Bombard-class attacks may legally deplete a city to exactly zero;
        // it remains standing until a melee-capable unit captures it.
        if city.hp < 0 {
            found.violation(
                "city below zero HP",
                format!("city {id} ({}) at {} HP", city.name, city.hp),
            );
        }
        let max_walls = g.city_max_wall_hp(city);
        if city.wall_hp > max_walls {
            found.violation(
                "walls above their pool",
                format!("city {id} ({}) at {}/{max_walls}", city.name, city.wall_hp),
            );
        }
        let total: f64 = city.pressure.values().sum::<f64>() + city.atheist_pressure;
        if total <= 0.0 {
            found.violation(
                "city with no religious standing at all",
                format!("city {id} ({})", city.name),
            );
        }

        // An empty queue is a city converting Production into nothing.
        if city.queue.is_empty() {
            let producible = g.producible_items(city.owner, *id);
            let military = g
                .player_unit_ids(city.owner)
                .into_iter()
                .filter(|unit| {
                    g.rules.units[g.units[unit].kind].class == "military"
                })
                .count();
            if bounded_minor_idle(g, city.owner, military, &producible) {
                // City-state governors build only their own district family,
                // repairs, and ordinary buildings. A general district or
                // repeatable project being engine-legal does not make it a
                // policy choice. Once the bounded garrison is full, calling
                // that deliberate stop a defect would pressure the AI to fill
                // the map solely to silence the auditor.
                history.city_idle_since.remove(id);
                continue;
            }
            let since = history.city_idle_since.entry(*id).or_insert(g.turn);
            if g.turn - *since >= IDLE_TURNS
                && !history.reported_city.get(id).copied().unwrap_or(false)
            {
                history.reported_city.insert(*id, true);
                let context = idle_city_context(g, *id);
                let role = controller_role(g, city.owner);
                found.symptom(
                    format!("{role} city builds nothing for {IDLE_TURNS}+ turns"),
                    format!(
                        "city {id} ({}) of {} idle since turn {since}{context}",
                        city.name, g.players[city.owner].civ,
                    ),
                );
            }
        } else {
            history.city_idle_since.remove(id);
        }
    }

    for player in &g.players {
        if player.is_barbarian {
            continue;
        }
        if player.alive && g.player_city_ids(player.id).is_empty() {
            let has_settler = g
                .player_unit_ids(player.id)
                .into_iter()
                .any(|unit| g.units[&unit].kind == "settler");
            if !has_settler {
                found.violation(
                    "player alive with no cities and no settler",
                    format!("player {} ({})", player.id, player.civ),
                );
            }
        }
        if player.gold < -f64::EPSILON {
            found.violation(
                "treasury below zero",
                format!("player {} ({}) at {}", player.id, player.civ, player.gold),
            );
        }
    }
}

/// End-of-game checks: things that are only wrong once the game is over.
fn audit_result(g: &Game, found: &mut Findings) {
    let Some(winner) = g.winner else {
        found.violation("game ended with no winner", String::new());
        return;
    };
    if g.players[winner].is_minor {
        found.violation(
            "a minor won the game",
            format!("{} took the game", g.players[winner].civ),
        );
    }
    if g.victory_type.is_none() {
        found.violation("winner with no victory type", String::new());
    }

    // The war log is only readable if a war is one entry. Two shipped rules
    // hold that: a war runs ten Standard turns before it can be settled, and
    // the peace binds for ten more. Both durations scale with game speed. A
    // record that breaks either means the log is filling with fragments of the
    // same war again.
    let war_minimum = g.standard_duration(WAR_MIN_TURNS);
    let treaty_minimum = g.standard_duration(WAR_MIN_TURNS);
    let mut previous: BTreeMap<(usize, usize), &WarRecord> = BTreeMap::new();
    for war in &g.concluded_wars {
        let key = (war.aggressor.min(war.defender), war.aggressor.max(war.defender));
        let Some(ended) = war.ended else { continue };
        let sides = (
            g.players[war.aggressor].civ.clone(),
            g.players[war.defender].civ.clone(),
        );
        if negotiated_war_ended_early(war, war_minimum) {
            found.violation(
                "a war ended before the shipped minimum",
                format!(
                    "{} against {} ran turns {}-{ended}",
                    sides.0, sides.1, war.started
                ),
            );
        }
        if let Some(previous_war) = previous.insert(key, war) {
            if redeclared_inside_peace_treaty(previous_war, war, treaty_minimum) {
                let last = previous_war.ended.unwrap_or_default();
                found.violation(
                    "the same pair re-declared inside the peace treaty",
                    format!(
                        "{} against {} again on turn {} after peace on turn {last}",
                        sides.0, sides.1, war.started
                    ),
                );
            }
        }
        if let Some((city, first, last)) = rapid_recapture_window(war) {
            found.symptom(
                "the same city is captured three times in ten turns",
                format!(
                    "{} repeatedly captured {city} from {} on turns {first}-{last}",
                    sides.0, sides.1
                ),
            );
        }
    }

    for player in &g.players {
        if player.is_barbarian || !player.alive {
            continue;
        }
        let cities = g.player_city_ids(player.id).len();
        // Treasury nobody ever spends is the signature of an AI that has run
        // out of things it knows how to buy. Legal unit actions alone are not
        // enough evidence: a saturated army can expose dozens of affordable
        // units that would only make the map more crowded. Count military
        // purchases only while the civilization still has a basic force gap.
        if treasury_looks_hoarded(player.is_minor, player.gold, player.gold_per_turn) {
            let mut purchasing = g.clone();
            purchasing.winner = None;
            purchasing.current = player.id;
            let military = purchasing
                .player_unit_ids(player.id)
                .into_iter()
                .filter(|unit| {
                    purchasing.rules.units[&purchasing.units[unit].kind].class == "military"
                })
                .count();
            let (units, buildings, districts) = purchasing
                .legal_actions(player.id)
                .into_iter()
                .fold((0, 0, 0), |mut counts, action| {
                    match action {
                        Action::Buy { unit, currency, .. }
                            if currency == "gold"
                                && military < cities.max(1)
                                && purchasing.rules.units[&unit].class == "military" =>
                        {
                            counts.0 += 1
                        }
                        Action::BuyBuilding { currency, .. } if currency == "gold" => {
                            counts.1 += 1
                        }
                        Action::BuyDistrict { currency, .. } if currency == "gold" => {
                            counts.2 += 1
                        }
                        _ => {}
                    }
                    counts
                });
            if units + buildings + districts == 0 {
                continue;
            }
            found.symptom(
                if player.is_minor {
                    "city-state hoards Gold it never spends"
                } else {
                    "civilization hoards Gold it never spends"
                },
                format!(
                    "{} finished on {:.0} Gold ({:+.1}/turn) with {cities} cities; useful affordable Gold purchases: {units} units, {buildings} buildings, {districts} districts",
                    player.civ, player.gold, player.gold_per_turn,
                ),
            );
        }
        if !player.is_minor && player.techs.len() <= 2 {
            found.symptom(
                "civilization researched almost nothing",
                format!("{} ended on {} techs", player.civ, player.techs.len()),
            );
        }
    }
    audit_tourism(g, found);
}

/// A culture victory is paid in *foreign* tourists, and every one of them is
/// pulled out of a rival's domestic pool by tourism pressure. Two of the
/// multipliers in `Game::international_tourism_multiplier` are not earned by
/// the tourist at all — they are granted by the civilization being toured.
/// Open Borders is worth +25%, and it is a diplomatic gift the host chooses
/// to make; a Trade Route into the host is worth another +25%. This census
/// asks, at the end of every game, how much of the tourism leader's reach was
/// bought with consent its hosts could have withheld.
///
/// It reports rather than judges. Granting Open Borders to the board's
/// tourism leader is a legal and often correct move — the point is to find
/// out whether it is a decision anyone is making.
fn granted_open_borders(g: &Game, source: usize, target: usize) -> bool {
    g.are_allied(source, target)
        || g.players[target]
            .open_borders_until
            .get(&source)
            .is_some_and(|until| *until > g.turn)
        || g.active_trade_deals
            .iter()
            .filter(|deal| deal.ends > g.turn)
            .any(|deal| {
                (deal.from == target && deal.to == source && deal.offer.open_borders)
                    || (deal.to == target && deal.from == source && deal.request.open_borders)
            })
}

fn tourism_trade_route(g: &Game, source: usize, target: usize) -> bool {
    g.routes.iter().any(|route| {
        route.owner == source
            && route.ends > g.turn
            && g.cities
                .get(&route.dest)
                .is_some_and(|city| city.owner == target)
    })
}

/// Which victory lanes the board's planners actually committed to.
///
/// `audit_tourism` can report that nobody came near the culture bar without
/// being able to say why. `AdvancedAi::strategy_census` counts the turns each
/// agent spent on each grand strategy, so summing it across the majors gives
/// the board's own answer: a lane nobody adopts is a lane nobody loses, and a
/// denial experiment run against it measures the arena rather than the agent.
fn audit_strategy_mix(g: &Game, ais: &[AdvancedAi], found: &mut Findings) {
    let mut lanes = [
        ("expansion", 0u32),
        ("science", 0),
        ("culture", 0),
        ("religion", 0),
        ("diplomacy", 0),
        ("conquest", 0),
        ("recovery", 0),
    ];
    for player in g
        .players
        .iter()
        .filter(|player| !player.is_minor && !player.is_barbarian)
    {
        let Some(census) = ais.get(player.id).map(|ai| ai.strategy_census()) else {
            continue;
        };
        lanes[0].1 += census.expansion;
        lanes[1].1 += census.science;
        lanes[2].1 += census.culture;
        lanes[3].1 += census.religion;
        lanes[4].1 += census.diplomacy;
        lanes[5].1 += census.conquest;
        lanes[6].1 += census.recovery;
    }
    let planned: u32 = lanes.iter().map(|(_, turns)| turns).sum();
    if planned == 0 {
        found.symptom("no major ever chose a grand strategy", String::new());
        return;
    }
    let mix = lanes
        .iter()
        .map(|(name, turns)| format!("{name} {}%", 100 * turns / planned))
        .collect::<Vec<_>>()
        .join(", ");
    // A victory lane that never attracts a twentieth of the board's planning
    // is not being contested, whatever the agent is capable of when told to
    // contest it.
    for (name, turns) in lanes
        .iter()
        .filter(|(name, _)| *name != "expansion" && *name != "recovery")
    {
        if turns * 20 < planned {
            found.symptom(
                format!("the board never contested the {name} lane"),
                format!(
                    "{name} took {}% of {planned} planner-turns; the board ran {mix}",
                    100 * turns / planned
                ),
            );
        }
    }
}

fn audit_tourism(g: &Game, found: &mut Findings) {
    let majors: Vec<usize> = g
        .players
        .iter()
        .filter(|p| p.alive && !p.is_minor && !p.is_barbarian)
        .map(|p| p.id)
        .collect();
    if majors.len() < 2 {
        return;
    }
    let Some(&leader) = majors.iter().max_by_key(|pid| g.foreign_tourists(**pid)) else {
        return;
    };
    let foreign = g.foreign_tourists(leader);
    if foreign <= 0 {
        found.symptom(
            "no civilization attracted a single foreign tourist",
            format!("best was {} on 0", g.players[leader].civ),
        );
        return;
    }
    let bar = majors
        .iter()
        .filter(|pid| **pid != leader && !g.same_team(leader, **pid))
        .map(|pid| g.domestic_tourists(*pid))
        .max()
        .unwrap_or(0);

    let hosts: Vec<usize> = majors
        .iter()
        .copied()
        .filter(|pid| *pid != leader && !g.same_team(leader, *pid))
        .collect();
    let consenting = hosts
        .iter()
        .filter(|host| granted_open_borders(g, leader, **host))
        .count();
    let routed = hosts
        .iter()
        .filter(|host| tourism_trade_route(g, leader, **host))
        .count();

    for host in &hosts {
        if granted_open_borders(g, leader, *host) {
            found.symptom(
                "the board's tourism leader toured on Open Borders its host granted",
                format!(
                    "{} granted {} Open Borders; {} ended on {} foreign tourists against a bar of {}",
                    g.players[*host].civ,
                    g.players[leader].civ,
                    g.players[leader].civ,
                    foreign,
                    bar,
                ),
            );
        }
    }

    // Report which side of the race this game landed on either way. A denial
    // experiment run in an arena that never produces the lane measures
    // nothing, and the only way to know that is to say so out loud: a census
    // that reports only its alarming case reads as quiet when it is blind.
    if foreign * 4 < bar {
        found.symptom(
            "the culture lane went uncontested",
            format!(
                "the board's best tourism was {} on {} foreign tourists against a bar of {} ({}%); {} of {} hosts had granted Open Borders",
                g.players[leader].civ,
                foreign,
                bar,
                if bar > 0 { 100 * foreign / bar } else { 0 },
                consenting,
                hosts.len(),
            ),
        );
    }
    if foreign * 4 >= bar * 3 {
        found.symptom(
            "a culture victory came within a quarter of landing",
            format!(
                "{} reached {} foreign tourists against a bar of {} ({} of {} hosts granted Open Borders, {} carried its Trade Routes)",
                g.players[leader].civ, foreign, bar, consenting, hosts.len(), routed,
            ),
        );
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let games = number(&args, "--games", 3).max(1);
    let start = number(&args, "--start-seed", 0);
    let players = number(&args, "--players", 8).max(1);
    let size = MapSize::for_players(players as usize);
    let width = number(&args, "--width", size.width as i64).max(8) as i32;
    let height = number(&args, "--height", size.height as i64).max(8) as i32;
    let city_states = number(&args, "--city-states", size.default_city_states as i64).max(0)
        as usize;
    let rules = Rules::embedded();
    let speed = text(&args, "--speed", &default_speed());
    let speed_turns = rules.speeds.get(&speed).unwrap_or_else(|| {
        eprintln!("unknown game speed {speed:?}");
        std::process::exit(2);
    });
    let turns = number(&args, "--turns", i64::from(speed_turns.turns)).max(1) as u32;
    let quiet = args.iter().any(|arg| arg == "--quiet");
    // ⚠⚠ WHICH AGENT IS BEING AUDITED. `AdvancedAi::fleet` is
    // `AdvancedAi::new()` per seat -- the STOCK controller, with no repair
    // bundle and no ledger. That is not what ships: the native deployment
    // genome is `enable_engine_repairs()`, which turns the repair universe on
    // and then applies the ledger's defaults. Until 2026-08-25 this binary
    // had no way to say so and no output that named it, so every number it
    // printed described the stock agent while reading as though it described
    // the agent. See `GENOME_READ` for what that cost.
    let genome = text(&args, "--genome", "deployment");

    let mut totals = Findings::default();
    let mut totals_motion = MotionBreakdown::default();
    for seed in start..start + games {
        let mut options = GameOptions::new(
            players as usize,
            width,
            height,
            seed as u64,
            turns,
            city_states,
        );
        options.speed = speed.clone();
        let mut g = Game::new_with(options);
        let mut ais = AdvancedAi::fleet(&g);
        if genome != "stock" {
            for ai in ais.iter_mut() {
                ai.enable_engine_repairs();
            }
        }
        let mut history = History::default();
        let mut found = Findings::default();
        let mut motion = MotionBreakdown::default();
        let mut last_turn = g.turn;
        while g.winner.is_none() {
            let pid = g.current;
            ais[pid].take_turn(&mut g, pid);
            if g.winner.is_none() && g.current == pid {
                let _ = g.apply(pid, &Action::EndTurn);
            }
            if g.turn != last_turn {
                last_turn = g.turn;
                audit_turn(&g, &mut history, &mut found, &mut motion);
            }
        }
        audit_result(&g, &mut found);
        audit_strategy_mix(&g, &ais, &mut found);
        for life in history.lives.values() {
            let entry = motion
                .major_lives
                .entry(life.kind.clone())
                .or_insert((0, 0, 0));
            entry.0 += 1;
            if !life.moved && !life.worked {
                entry.1 += 1;
                if life.turns >= NEVER_ACTED_MIN_TURNS {
                    entry.2 += 1;
                }
            }
        }
        totals_motion.add(&motion);

        if !quiet {
            println!(
                "seed {seed:<5} t{:<4} {:<10} {:<10} violations={} symptoms={}",
                g.reported_turn(),
                g.victory_type.clone().unwrap_or_default(),
                g.winner.map(|w| g.players[w].civ.clone()).unwrap_or_default(),
                found.violations.values().map(|entry| entry.0).sum::<usize>(),
                found.symptoms.values().map(|entry| entry.0).sum::<usize>(),
            );
            motion.print("    ");
            for (key, (count, detail)) in &found.violations {
                println!("    VIOLATION x{count:<5} {key} - e.g. {detail}");
            }
            for (key, (count, detail)) in &found.symptoms {
                println!("    symptom   x{count:<5} {key} - e.g. {detail}");
            }
        }
        for (key, (count, detail)) in found.violations {
            let entry = totals
                .violations
                .entry(key)
                .or_insert_with(|| (0, detail.clone()));
            entry.0 += count;
        }
        for (key, (count, detail)) in found.symptoms {
            let entry = totals
                .symptoms
                .entry(key)
                .or_insert_with(|| (0, detail.clone()));
            entry.0 += count;
        }
    }

    println!("\n=== {games} games ===");
    println!(
        "profile   {players}p {width}x{height}, {speed}, {turns} turns, {city_states} city-states, seeds {start}..{}",
        start + games - 1,
    );
    // ⚠ A report that does not name its genome reads as though it described
    // the agent, and for months this one described the stock controller. See
    // the module header.
    println!(
        "genome    {}",
        if genome == "stock" {
            "stock — AdvancedAi::new(), no repair bundle, no ledger (--genome stock)"
        } else {
            "deployment — enable_engine_repairs(): the repair universe, then the ledger"
        }
    );
    totals_motion.print("");
    if totals.violations.is_empty() {
        println!("no rule violations");
    }
    for (key, (count, detail)) in &totals.violations {
        println!("VIOLATION x{count:<6} {key} - e.g. {detail}");
    }
    for (key, (count, detail)) in &totals.symptoms {
        println!("symptom   x{count:<6} {key} - e.g. {detail}");
    }
    if !totals.violations.is_empty() {
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use civvis::game::{Item, WarHighlight, WarRecord};

    use super::{
        bounded_minor_idle, city_state_district_family, granted_open_borders,
        negotiated_war_ended_early, rapid_recapture_window, redeclared_inside_peace_treaty,
        tourism_trade_route, track_unit, treasury_looks_hoarded, unit_had_idle_opportunity,
        History, Motion, LIVELOCK_TURNS,
    };
    use civvis::game::{Action, ActiveTradeDeal, DealItems, Game, TradeRoute};

    /// Walk one unit through a scripted sequence of tiles, one per turn, and
    /// report how many livelock episodes the auditor opened.
    fn walk(tiles: &[usize], spend_charge_on: Option<usize>) -> (usize, Motion) {
        let mut g = Game::new(2, 24, 16, 11, 300, 0);
        let id = *g.units.keys().next().unwrap();
        let ground: Vec<civvis::Pos> = {
            let mut all: Vec<civvis::Pos> = g.map.tiles.keys().copied().collect();
            all.sort();
            all
        };
        let mut history = History::default();
        let mut motion = Motion::default();
        let mut reports = 0;
        for (step, tile) in tiles.iter().enumerate() {
            g.turn += 1;
            g.units.get_mut(&id).unwrap().pos = ground[*tile];
            if spend_charge_on == Some(step) {
                g.units.get_mut(&id).unwrap().charges += 1;
            }
            if track_unit(&g, &mut history, id, &mut motion).is_some() {
                reports += 1;
            }
        }
        (reports, motion)
    }

    #[test]
    fn a_unit_shuttling_between_two_tiles_is_reported_once_and_keeps_counting() {
        let shuttle: Vec<usize> = (0..30).map(|turn| turn % 2).collect();
        let (reports, motion) = walk(&shuttle, None);
        assert_eq!(reports, 1, "one episode, however long it runs");
        assert!(
            motion.livelock >= 30 - LIVELOCK_TURNS as u64 - 1,
            "every turn past the threshold counts toward the share: {}",
            motion.livelock
        );
        assert_eq!(motion.unit_turns, 30);
    }

    #[test]
    fn a_unit_that_is_actually_travelling_is_never_reported() {
        let march: Vec<usize> = (0..30).collect();
        let (reports, motion) = walk(&march, None);
        assert_eq!(reports, 0);
        assert_eq!(motion.livelock, 0);
    }

    #[test]
    fn a_three_tile_circuit_is_a_livelock_but_a_four_tile_one_is_a_patrol() {
        let (circuit, _) = walk(&(0..30).map(|turn| turn % 3).collect::<Vec<_>>(), None);
        assert_eq!(circuit, 1);
        let (patrol, _) = walk(&(0..30).map(|turn| turn % 4).collect::<Vec<_>>(), None);
        assert_eq!(patrol, 0);
    }

    #[test]
    fn work_done_midway_clears_the_episode() {
        let shuttle: Vec<usize> = (0..20).map(|turn| turn % 2).collect();
        // Without the charge this shuttle reports; spending one at the halfway
        // point restarts the episode and neither half is long enough.
        assert_eq!(walk(&shuttle, None).0, 1);
        assert_eq!(walk(&shuttle, Some(10)).0, 0);
    }

    #[test]
    fn a_unit_standing_still_in_the_open_counts_as_idle_not_as_livelock() {
        let (reports, motion) = walk(&vec![0; 30], None);
        assert_eq!(reports, 0, "a unit that stopped is a stall, not a circle");
        assert_eq!(motion.livelock, 0);
        assert!(motion.idle_field + motion.picket >= 29);
    }

    fn concluded_war(kind: &str) -> WarRecord {
        WarRecord {
            conflict: 1,
            declarer: 0,
            target: 1,
            casus_belli: None,
            joint_war_until: None,
            aggressor: 0,
            defender: 1,
            started: 20,
            ended: Some(24),
            losses: BTreeMap::new(),
            participants: Vec::new(),
            peace_terms: Vec::new(),
            highlights: vec![
                WarHighlight {
                    turn: 20,
                    kind: "declared".to_string(),
                    actor: 0,
                    subject: 1,
                    city: None,
                },
                WarHighlight {
                    turn: 24,
                    kind: kind.to_string(),
                    actor: 0,
                    subject: 1,
                    city: None,
                },
            ],
            theater: Vec::new(),
        }
    }

    #[test]
    fn the_war_minimum_applies_only_to_negotiated_peace() {
        assert!(negotiated_war_ended_early(&concluded_war("peace"), 10));
        assert!(!negotiated_war_ended_early(
            &concluded_war("conquest"),
            10
        ));
        assert!(!negotiated_war_ended_early(
            &concluded_war("coalition"),
            10
        ));
        let mut online_length = concluded_war("peace");
        online_length.ended = Some(28);
        online_length.highlights[1].turn = 28;
        assert!(
            !negotiated_war_ended_early(&online_length, 8),
            "Online scales the shipped ten-turn minimum to eight"
        );
    }

    #[test]
    fn the_treaty_cooldown_applies_only_after_negotiated_peace() {
        let mut next = concluded_war("peace");
        next.started = 25;
        next.ended = Some(35);
        next.highlights[0].turn = 25;

        assert!(redeclared_inside_peace_treaty(
            &concluded_war("peace"),
            &next,
            10,
        ));
        assert!(!redeclared_inside_peace_treaty(
            &concluded_war("coalition"),
            &next,
            10,
        ));
        assert!(!redeclared_inside_peace_treaty(
            &concluded_war("conquest"),
            &next,
            10,
        ));
        let mut after_scaled_treaty = next.clone();
        after_scaled_treaty.started = 32;
        assert!(!redeclared_inside_peace_treaty(
            &concluded_war("peace"),
            &after_scaled_treaty,
            8,
        ));
    }

    #[test]
    fn rapid_loyalty_recaptures_are_visible_to_the_auditor() {
        let mut war = concluded_war("peace");
        war.highlights.splice(
            1..1,
            [20, 24, 27].map(|turn| WarHighlight {
                turn,
                kind: "capital_captured".to_string(),
                actor: 0,
                subject: 1,
                city: Some("Loop City".to_string()),
            }),
        );
        assert_eq!(
            rapid_recapture_window(&war),
            Some(("Loop City".to_string(), 20, 27))
        );
    }

    #[test]
    fn bounded_city_state_garrisons_are_not_reported_as_idle_production() {
        let g = Game::new_full(2, 24, 16, 97, 120, 1, false);
        let minor = g
            .players
            .iter()
            .find(|player| player.is_minor && !player.is_barbarian)
            .map(|player| player.id)
            .unwrap();
        let units = vec![
            Item::Unit {
                unit: civvis::name!("builder"),
            },
            Item::Unit {
                unit: civvis::name!("warrior"),
            },
        ];
        assert!(bounded_minor_idle(&g, minor, 3, &units));
        assert!(!bounded_minor_idle(&g, 0, 3, &units));
        assert!(!bounded_minor_idle(&g, minor, 2, &units));

        let mut investment = units;
        investment.push(Item::Building {
            building: civvis::name!("monument"),
        });
        assert!(!bounded_minor_idle(&g, minor, 3, &investment));

        let specialty = city_state_district_family(&g, minor);
        let specialty = Item::District {
            district: civvis::name::Name::new(specialty),
            pos: (0, 0),
        };
        assert!(!bounded_minor_idle(&g, minor, 3, &[specialty]));

        // A district outside this city-state's specialty is legal engine
        // output, but not a choice its deliberately narrow governor makes.
        let other = ["campus", "commercial_hub", "theater_square"]
            .into_iter()
            .find(|family| *family != city_state_district_family(&g, minor))
            .unwrap();
        let general = Item::District {
            district: civvis::name::Name::new(other),
            pos: (0, 0),
        };
        assert!(bounded_minor_idle(&g, minor, 3, &[general]));
    }

    #[test]
    fn a_trader_gets_a_real_turn_after_capacity_opens() {
        let mut history = History::default();
        assert!(!unit_had_idle_opportunity(
            &mut history,
            7,
            "trader",
            20,
            1,
            1,
            false,
        ));
        assert!(!unit_had_idle_opportunity(
            &mut history,
            7,
            "trader",
            21,
            0,
            1,
            true,
        ));
        assert!(unit_had_idle_opportunity(
            &mut history,
            7,
            "trader",
            22,
            0,
            1,
            true,
        ));
        assert!(!unit_had_idle_opportunity(
            &mut history,
            7,
            "trader",
            23,
            0,
            1,
            false,
        ));
        assert!(unit_had_idle_opportunity(
            &mut history,
            8,
            "builder",
            20,
            1,
            1,
            false,
        ));
    }

    #[test]
    fn treasury_warning_scales_with_a_major_empires_income() {
        assert!(treasury_looks_hoarded(false, 8_797.0, 232.4));
        assert!(!treasury_looks_hoarded(false, 1_409.0, 199.0));
        assert!(treasury_looks_hoarded(true, 1_340.0, 22.3));
    }

    /// `granted_open_borders` mirrors the private `Game::tourism_open_borders`,
    /// so pin all three ways the engine reaches that `true` — and both ways it
    /// expires. A census that silently stops agreeing with the rule it is
    /// auditing reports zeros and reads like good news.
    #[test]
    fn the_census_sees_every_route_to_open_borders() {
        let mut g = Game::new(2, 24, 16, 11, 300, 0);
        g.turn = 40;
        assert!(
            !granted_open_borders(&g, 0, 1),
            "a fresh game grants nothing"
        );

        // The grant is recorded on the *host*, keyed by the visitor.
        g.players[1].open_borders_until.insert(0, 45);
        assert!(granted_open_borders(&g, 0, 1), "host 1 let visitor 0 in");
        assert!(
            !granted_open_borders(&g, 1, 0),
            "the grant is one-directional"
        );
        g.players[1].open_borders_until.insert(0, 40);
        assert!(
            !granted_open_borders(&g, 0, 1),
            "the grant expires on its turn"
        );
        g.players[1].open_borders_until.remove(&0);

        // A standing deal carries the same permission either way round.
        let offered = DealItems {
            open_borders: true,
            ..Default::default()
        };
        g.active_trade_deals.push(ActiveTradeDeal {
            id: 1,
            from: 1,
            to: 0,
            offer: offered,
            request: DealItems::default(),
            started: 30,
            ends: 50,
        });
        assert!(
            granted_open_borders(&g, 0, 1),
            "host 1 offered it in a deal"
        );
        g.active_trade_deals[0].ends = 40;
        assert!(
            !granted_open_borders(&g, 0, 1),
            "an ended deal grants nothing"
        );
        g.active_trade_deals.clear();

        let requested = DealItems {
            open_borders: true,
            ..Default::default()
        };
        g.active_trade_deals.push(ActiveTradeDeal {
            id: 2,
            from: 0,
            to: 1,
            offer: DealItems::default(),
            request: requested,
            started: 30,
            ends: 50,
        });
        assert!(
            granted_open_borders(&g, 0, 1),
            "visitor 0 requested it of host 1"
        );
    }

    /// The other granted multiplier. A Trade Route only carries tourism while
    /// it is running and only against the civilization that owns the
    /// destination city.
    #[test]
    fn the_census_counts_only_live_routes_into_the_host() {
        let mut g = Game::new(2, 24, 16, 11, 300, 0);
        let settler = *g
            .units
            .iter()
            .find(|(_, unit)| unit.owner == 1 && unit.kind == "settler")
            .map(|(id, _)| id)
            .expect("player 1 starts with a settler");
        g.current = 1;
        g.apply(1, &Action::FoundCity { unit: settler })
            .expect("a starting settler can found");
        let city = *g
            .cities
            .iter()
            .find(|(_, city)| city.owner == 1)
            .map(|(id, _)| id)
            .expect("player 1 now holds a city");
        g.turn = 40;

        assert!(!tourism_trade_route(&g, 0, 1), "no routes yet");
        g.routes.push(TradeRoute {
            origin: 0,
            dest: city,
            owner: 0,
            ends: 50,
        });
        assert!(
            tourism_trade_route(&g, 0, 1),
            "player 0 trades into player 1"
        );
        assert!(
            !tourism_trade_route(&g, 1, 0),
            "the route is one-directional"
        );
        g.routes[0].ends = 40;
        assert!(
            !tourism_trade_route(&g, 0, 1),
            "an expired route carries nothing"
        );
        g.routes[0].ends = 50;
        g.routes[0].dest = city + 9_999;
        assert!(
            !tourism_trade_route(&g, 0, 1),
            "a route to no city carries nothing"
        );
    }
}
