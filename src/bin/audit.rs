//! Plays full games and reports what looked wrong while they ran.
//!
//! `soak` answers "did the game finish"; this answers "was the game any
//! good". It walks every turn of a live game and records both hard invariant
//! breaks (state the rules should never produce) and soft symptoms - idle
//! units, cities producing nothing, treasuries nobody spends - which are the
//! shape most engine and AI defects actually take from the outside.
//!
//! Usage: audit [--games N] [--start-seed N] [--players N] [--turns N]
//!              [--width N] [--height N] [--city-states N] [--quiet]
use std::collections::{BTreeMap, HashSet};

use civvis::ai::{AdvancedAi, Ai};
use civvis::game::{Action, Game, WarRecord};
use civvis::setup::MapSize;

fn number(args: &[String], flag: &str, default: i64) -> i64 {
    args.iter()
        .position(|arg| arg == flag)
        .and_then(|index| args.get(index + 1))
        .and_then(|value| value.parse().ok())
        .unwrap_or(default)
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
fn negotiated_war_ended_early(war: &WarRecord) -> bool {
    let Some(ended) = war.ended else {
        return false;
    };
    war.highlights
        .last()
        .is_some_and(|highlight| highlight.kind == "peace")
        && ended.saturating_sub(war.started) < WAR_MIN_TURNS
}

/// Only negotiated peace creates the treaty whose cooldown this audit checks.
/// Emergency coalitions can compel two members to stop fighting, and conquest
/// can close a ledger record too; neither is a peace agreement between the
/// pair, so a later war must not be reported as violating a treaty that never
/// existed.
fn redeclared_inside_peace_treaty(previous: &WarRecord, next: &WarRecord) -> bool {
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
        && next.started.saturating_sub(ended) < WAR_MIN_TURNS
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
    /// Stood still in the open, fortified. A picket is legitimate; a
    /// stampede into this column is a livelock fix that only hid the problem.
    picket: u64,
}

impl Motion {
    fn add(&mut self, other: Motion) {
        self.unit_turns += other.unit_turns;
        self.livelock += other.livelock;
        self.idle_field += other.idle_field;
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
            "unit-turns={} livelock={} ({:.2}%) idle-field={} ({:.2}%) picket={} ({:.2}%)",
            self.unit_turns,
            self.livelock,
            rate(self.livelock),
            self.idle_field,
            rate(self.idle_field),
            self.picket,
            rate(self.picket),
        )
    }
}

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
        g.players[pid].techs.contains("shipbuilding"),
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

fn bounded_minor_idle(
    player_is_minor: bool,
    military: usize,
    producible: &[civvis::game::Item],
) -> bool {
    player_is_minor
        && military >= 3
        && !producible.is_empty()
        && producible
            .iter()
            .all(|item| matches!(item, civvis::game::Item::Unit { .. }))
}

fn audit_turn(g: &Game, history: &mut History, found: &mut Findings, motion: &mut Motion) {
    history.tracks.retain(|id, _| g.units.contains_key(id));
    for (id, unit) in &g.units {
        if let Some(detail) = track_unit(g, history, *id, motion) {
            found.symptom(
                format!("{} circles without progress {LIVELOCK_TURNS}+ turns", unit.kind),
                detail,
            );
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
                format!("{} sits still {IDLE_TURNS}+ turns", unit.kind),
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
                    g.rules.units[g.units[unit].kind.as_str()].class == "military"
                })
                .count();
            if bounded_minor_idle(g.players[city.owner].is_minor, military, &producible) {
                // A one-city state with no remaining infrastructure or
                // project site deliberately stops at its bounded garrison.
                // Calling that a production defect would pressure the AI to
                // fill the map with units solely to silence the auditor.
                history.city_idle_since.remove(id);
                continue;
            }
            let since = history.city_idle_since.entry(*id).or_insert(g.turn);
            if g.turn - *since >= IDLE_TURNS
                && !history.reported_city.get(id).copied().unwrap_or(false)
            {
                history.reported_city.insert(*id, true);
                let context = idle_city_context(g, *id);
                found.symptom(
                    "city builds nothing for 25+ turns",
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
    // hold that: a war runs ten turns before it can be settled, and the peace
    // binds for ten more. A record that breaks either means the log is filling
    // with fragments of the same war again.
    let mut previous: BTreeMap<(usize, usize), &WarRecord> = BTreeMap::new();
    for war in &g.concluded_wars {
        let key = (war.aggressor.min(war.defender), war.aggressor.max(war.defender));
        let Some(ended) = war.ended else { continue };
        let sides = (
            g.players[war.aggressor].civ.clone(),
            g.players[war.defender].civ.clone(),
        );
        if negotiated_war_ended_early(war) {
            found.violation(
                "a war ended before the shipped minimum",
                format!(
                    "{} against {} ran turns {}-{ended}",
                    sides.0, sides.1, war.started
                ),
            );
        }
        if let Some(previous_war) = previous.insert(key, war) {
            if redeclared_inside_peace_treaty(previous_war, war) {
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
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let games = number(&args, "--games", 3);
    let start = number(&args, "--start-seed", 0);
    let players = number(&args, "--players", 8).max(1);
    let size = MapSize::for_players(players as usize);
    let width = number(&args, "--width", size.width as i64) as i32;
    let height = number(&args, "--height", size.height as i64) as i32;
    let city_states = number(&args, "--city-states", size.default_city_states as i64) as usize;
    let turns = number(&args, "--turns", 300) as u32;
    let quiet = args.iter().any(|arg| arg == "--quiet");

    let mut totals = Findings::default();
    let mut totals_motion = Motion::default();
    for seed in start..start + games {
        let mut g = Game::new(
            players as usize,
            width,
            height,
            seed as u64,
            turns,
            city_states,
        );
        let mut ais = AdvancedAi::fleet(&g);
        let mut history = History::default();
        let mut found = Findings::default();
        let mut motion = Motion::default();
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
        totals_motion.add(motion);

        if !quiet {
            println!(
                "seed {seed:<5} t{:<4} {:<10} {:<10} violations={} symptoms={}",
                g.turn,
                g.victory_type.clone().unwrap_or_default(),
                g.winner.map(|w| g.players[w].civ.clone()).unwrap_or_default(),
                found.violations.values().map(|entry| entry.0).sum::<usize>(),
                found.symptoms.values().map(|entry| entry.0).sum::<usize>(),
            );
            println!("    motion    {}", motion.line());
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
    println!("motion    {}", totals_motion.line());
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
        bounded_minor_idle, negotiated_war_ended_early, rapid_recapture_window,
        redeclared_inside_peace_treaty, track_unit, treasury_looks_hoarded,
        unit_had_idle_opportunity, History, Motion, LIVELOCK_TURNS,
    };
    use civvis::game::Game;

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
        assert!(negotiated_war_ended_early(&concluded_war("peace")));
        assert!(!negotiated_war_ended_early(&concluded_war("conquest")));
        assert!(!negotiated_war_ended_early(&concluded_war("coalition")));
    }

    #[test]
    fn the_treaty_cooldown_applies_only_after_negotiated_peace() {
        let mut next = concluded_war("peace");
        next.started = 25;
        next.ended = Some(35);
        next.highlights[0].turn = 25;

        assert!(redeclared_inside_peace_treaty(
            &concluded_war("peace"),
            &next
        ));
        assert!(!redeclared_inside_peace_treaty(
            &concluded_war("coalition"),
            &next
        ));
        assert!(!redeclared_inside_peace_treaty(
            &concluded_war("conquest"),
            &next
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
        let units = vec![
            Item::Unit {
                unit: "builder".to_string(),
            },
            Item::Unit {
                unit: "warrior".to_string(),
            },
        ];
        assert!(bounded_minor_idle(true, 3, &units));
        assert!(!bounded_minor_idle(false, 3, &units));
        assert!(!bounded_minor_idle(true, 2, &units));

        let mut investment = units;
        investment.push(Item::Project {
            project: "campus_research_grants".to_string(),
        });
        assert!(!bounded_minor_idle(true, 3, &investment));
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
}
