//! Same-seed policy probe for one Gran Colombia Domination seat against
//! three adaptive CIVVIS rivals. This is a simulator probe, not native Civ VI
//! verification or a deployment-ledger screen.
//!
//! victory_eval --domination-pair siege-is-progress-3 --games 16 \
//!   --start-seed 37140000 --out /tmp/domination-pairs.jsonl
//!
//! Both legs use 4 players, 60x38 Pangaea, 6 city-states, Online,
//! barbarians, all victory conditions and the natural 250-turn clock.
//! `--difficulty prince|king` sets both player and barbarian difficulty.
//! Omission preserves the historical King-player/Emperor-barbarian profile.
//! Only seat zero's named policy is disabled/enabled. The other seats keep
//! their adaptive deployed controllers. The focal seat also carries the
//! repository's compiled live-force-on bundle, recorded in every pair.
use civvis::ai::{gene, run_game_observed, AdvancedAi, Ai, Gene, VictoryTarget};
use civvis::game::{Action, Game, GameOptions, LeaderPool};
use civvis::setup::MapScript;
use serde::Serialize;
use std::collections::BTreeSet;
use std::fs::OpenOptions;
use std::io::{BufWriter, Write};
use std::path::PathBuf;

const FORCED: &str = include_str!("../../../deploy/live-force-on.txt");

struct Config {
    policy: &'static Gene,
    difficulty: Option<&'static str>,
    games: u64,
    start_seed: u64,
    out: PathBuf,
}

fn parse(args: &[String]) -> Result<Config, String> {
    let mut policy = None;
    let mut difficulty = None;
    let mut games = 16;
    let mut start_seed = 37_140_000;
    let mut out = None;
    let mut seen = BTreeSet::new();
    for pair in args.chunks(2) {
        let flag = &pair[0];
        if !seen.insert(flag) {
            return Err(format!("duplicate argument {flag}"));
        }
        let value = pair.get(1).ok_or_else(|| format!("{flag} needs a value"))?;
        match flag.as_str() {
            "--domination-pair" => {
                policy = Some(gene(value).ok_or_else(|| format!("unknown policy {value}"))?);
            }
            "--difficulty" => {
                difficulty = Some(match value.as_str() {
                    "prince" => "prince",
                    "king" => "king",
                    _ => return Err("--difficulty must be prince or king".to_string()),
                });
            }
            "--games" => games = value.parse::<u64>().map_err(|e| e.to_string())?,
            "--start-seed" => start_seed = value.parse::<u64>().map_err(|e| e.to_string())?,
            "--out" => out = Some(PathBuf::from(value)),
            _ => {
                return Err(format!(
                    "unsupported argument {flag} for fixed-profile Domination probe"
                ))
            }
        }
    }
    if games == 0 || start_seed.checked_add(games - 1).is_none() {
        return Err("games must be positive and the seed range must fit u64".to_string());
    }
    Ok(Config {
        policy: policy.ok_or("--domination-pair is required")?,
        difficulty,
        games,
        start_seed,
        out: out.ok_or("--out is required; existing files are never overwritten")?,
    })
}

fn options(seed: u64, difficulty: Option<&str>) -> GameOptions {
    GameOptions {
        map_script: MapScript::Pangaea,
        difficulty: difficulty.unwrap_or("king").to_string(),
        // Explicit native-rung probes must not inherit the engine’s default
        // Emperor barbarians. Keep omission compatible with archived probes.
        barbarian_difficulty: difficulty.unwrap_or("emperor").to_string(),
        speed: "online".to_string(),
        civs: vec!["Gran Colombia".to_string()],
        leader_pool: LeaderPool::Civ6,
        randomize_civs: true,
        handicap_exempt: BTreeSet::from([0]),
        barbarians: true,
        // Shipped Civ VI Maps.xml:106: MAPSIZE_TINY, four players, 60x38.
        ..GameOptions::new(4, 60, 38, seed, 250, 6)
    }
}

fn profile(seed: u64, difficulty: Option<&str>) -> serde_json::Value {
    let setup = options(seed, difficulty);
    serde_json::json!({
        "players": setup.players, "width": setup.width, "height": setup.height,
        "map": format!("{:?}", setup.map_script), "difficulty": setup.difficulty,
        "speed": setup.speed, "max_turns": setup.max_turns,
        "city_states": setup.city_states, "barbarians": setup.barbarians,
        "barbarian_difficulty": setup.barbarian_difficulty,
        "handicap_exempt": setup.handicap_exempt, "human_seats": setup.human_seats,
        "leader_pool": setup.leader_pool.id(), "civs": setup.civs,
        "randomize_civs": setup.randomize_civs, "victories": setup.victory_conditions,
        "mercy_rule": setup.mercy_rule, "required_victory_types": setup.required_victory_types,
        "game_modes": setup.game_modes, "teams": setup.teams,
        "disaster_intensity": setup.disaster_intensity,
        "base_ruleset": format!("{:?}", setup.base_ruleset),
        "future_era": format!("{:?}", setup.future_era), "start_era": setup.start_era,
        "turn_structure": format!("{:?}", setup.turn_structure),
        "map_topology": format!("{:?}", setup.map_topology),
        "map_poles": format!("{:?}", setup.map_poles),
    })
}

fn fleet(game: &Game, policy: &Gene, enabled: bool) -> Vec<AdvancedAi> {
    let mut ais = AdvancedAi::fleet(game);
    for ai in &mut ais {
        ai.enable_live_bridge();
    }
    ais[0] = AdvancedAi::targeting(VictoryTarget::Domination);
    ais[0].enable_live_bridge_universe();
    let forced: Vec<&str> = FORCED.trim().split(',').collect();
    ais[0].apply_gene_ledger_with_forced_live(&forced);
    if enabled {
        (policy.enable)(&mut ais[0]);
    } else {
        (policy.disable)(&mut ais[0]);
    }
    ais
}

#[derive(Debug, Serialize)]
struct Outcome {
    winner: Option<usize>,
    victory: Option<String>,
    turn: u32,
    focal_won: bool,
    focal_domination_won: bool,
    focal_score: i64,
    foreign_cities_observed_held: usize,
    foreign_cities_held_at_end: usize,
    applied_actions: usize,
    conquest: ConquestProgress,
    /// The focal seat's air-surge census at the end of the game.
    air_surge: String,
    /// The focal seat's economy beside the strongest rival's at
    /// `ECONOMY_MARKS`, so a build-order policy is read on the axis it moves.
    economy: Vec<EconomySnapshot>,
    /// Turns the focal seat spent bankrupt (its bankruptcy Amenity penalty
    /// above zero), so a treasury policy is read on the failure it guards.
    bankrupt_turns: u32,
}

/// Turns at which [`EconomySnapshot`] is taken: the live ladder's own
/// checkpoints (`cities_at_60`, `techs_at_100`) and a later one.
/// Schema 5 adds turns 40, 80, 120 and 200 so a production policy is read on
/// the curve it bends, not only at the ladder's checkpoints.
const ECONOMY_MARKS: [u32; 10] = [40, 60, 80, 100, 120, 140, 150, 160, 180, 200];

#[derive(Debug, Default, Serialize, Clone)]
struct SeatEconomy {
    techs: usize,
    civics: usize,
    treasury: f64,
    science: f64,
    culture: f64,
    gold: f64,
    population: i64,
    cities: usize,
    districts: usize,
    monuments: usize,
    granaries: usize,
    libraries: usize,
    /// City production a turn (handicap included, as the engine pays it).
    production: f64,
    /// Every turn's city production summed from turn one to this mark.
    production_to_date: f64,
    industrial_zones: usize,
    workshops: usize,
    factories: usize,
    power_plants: usize,
    military_units: usize,
    /// Live trade routes other majors own that end in this seat's cities:
    /// each grants its owner +25% Tourism against this seat.
    inbound_foreign_routes: usize,
}

impl SeatEconomy {
    fn of(g: &Game, pid: usize, production_to_date: f64) -> Self {
        let mut seat = SeatEconomy {
            techs: g.players[pid].techs.len(),
            civics: g.players[pid].civics.len(),
            treasury: g.players[pid].gold,
            production_to_date,
            military_units: g
                .units
                .values()
                .filter(|unit| unit.owner == pid && g.rules.units[unit.kind].class == "military")
                .count(),
            inbound_foreign_routes: g
                .routes
                .iter()
                .filter(|route| {
                    route.owner != pid
                        && route.ends > g.turn
                        && !g.players[route.owner].is_minor
                        && g.cities
                            .get(&route.dest)
                            .is_some_and(|city| city.owner == pid)
                })
                .count(),
            ..Default::default()
        };
        for city in g.cities.values().filter(|city| city.owner == pid) {
            let yields = g.city_yields(city.id);
            seat.science += yields.science;
            seat.culture += yields.culture;
            seat.gold += yields.gold;
            seat.production += yields.production;
            seat.population += i64::from(city.pop);
            seat.cities += 1;
            seat.districts += city
                .districts
                .keys()
                .filter(|district| g.rules.districts[district].specialty)
                .count();
            let has = |name: &str| city.buildings.iter().any(|b| b.as_str() == name);
            seat.monuments += usize::from(has("monument"));
            seat.granaries += usize::from(has("granary"));
            seat.libraries += usize::from(has("library"));
            seat.industrial_zones +=
                usize::from(g.city_has_district_family(city, civvis::name!("industrial_zone")));
            seat.workshops += usize::from(has("workshop"));
            seat.factories += usize::from(has("factory") || has("electronics_factory"));
            seat.power_plants += usize::from(
                has("coal_power_plant") || has("oil_power_plant") || has("nuclear_power_plant"),
            );
        }
        seat
    }

    /// Field-wise maximum, so `best_rival` reads the strongest rival on each
    /// axis rather than one rival on all of them.
    fn max(self, other: Self) -> Self {
        SeatEconomy {
            techs: self.techs.max(other.techs),
            civics: self.civics.max(other.civics),
            treasury: self.treasury.max(other.treasury),
            science: self.science.max(other.science),
            culture: self.culture.max(other.culture),
            gold: self.gold.max(other.gold),
            population: self.population.max(other.population),
            cities: self.cities.max(other.cities),
            districts: self.districts.max(other.districts),
            monuments: self.monuments.max(other.monuments),
            granaries: self.granaries.max(other.granaries),
            libraries: self.libraries.max(other.libraries),
            production: self.production.max(other.production),
            production_to_date: self.production_to_date.max(other.production_to_date),
            industrial_zones: self.industrial_zones.max(other.industrial_zones),
            workshops: self.workshops.max(other.workshops),
            factories: self.factories.max(other.factories),
            power_plants: self.power_plants.max(other.power_plants),
            military_units: self.military_units.max(other.military_units),
            inbound_foreign_routes: self
                .inbound_foreign_routes
                .max(other.inbound_foreign_routes),
        }
    }
}

#[derive(Debug, Serialize)]
struct EconomySnapshot {
    turn: u32,
    focal: SeatEconomy,
    best_rival: SeatEconomy,
    /// Every living major's `(seat, domestic tourists, foreign tourists)`:
    /// a Culture victory is foreign tourists above every other major's
    /// domestic count, so this names who holds the bar.
    tourists: Vec<(usize, i64, i64)>,
}

/// Adds each seat's city production for this turn to its running total.
/// Called once per observed turn boundary.
fn accrue_production(g: &Game, totals: &mut Vec<f64>) {
    totals.resize(g.players.len(), 0.0);
    for city in g.cities.values() {
        if let Some(total) = totals.get_mut(city.owner) {
            *total += g.city_yields(city.id).production;
        }
    }
}

fn observe_economy(g: &Game, totals: &[f64], economy: &mut Vec<EconomySnapshot>) {
    let Some(&mark) = ECONOMY_MARKS.get(economy.len()) else {
        return;
    };
    if g.turn < mark {
        return;
    }
    let to_date = |pid: usize| totals.get(pid).copied().unwrap_or(0.0);
    let best_rival = g
        .players
        .iter()
        .filter(|p| p.id != 0 && p.alive && !p.is_minor && !p.is_barbarian)
        .map(|p| SeatEconomy::of(g, p.id, to_date(p.id)))
        .fold(SeatEconomy::default(), SeatEconomy::max);
    let tourists = g
        .players
        .iter()
        .filter(|p| p.alive && !p.is_minor && !p.is_barbarian)
        .map(|p| (p.id, g.domestic_tourists(p.id), g.foreign_tourists(p.id)))
        .collect();
    economy.push(EconomySnapshot {
        turn: g.turn,
        focal: SeatEconomy::of(g, 0, to_date(0)),
        best_rival,
        tourists,
    });
}

/// Observations occur at turn boundaries and once at the end. These times are
/// first *observed* turns, not exact action timestamps; transient ownership
/// between observations is deliberately not counted as a held city.
#[derive(Debug, Default, Serialize)]
struct ConquestProgress {
    first_major_war_observed_turn: Option<u32>,
    first_focal_major_declaration_observed_turn: Option<u32>,
    focal_major_declarations: usize,
    focal_minor_declarations: usize,
    first_major_city_held_observed_turn: Option<u32>,
    first_foreign_capital_held_observed_turn: Option<u32>,
    foreign_major_cities_observed_held: BTreeSet<u32>,
    foreign_capitals_observed_held: BTreeSet<u32>,
    foreign_capitals_held_at_end: usize,
    own_original_capital_held_at_end: bool,
    /// The air lane: when the focal seat first held Flight and Advanced
    /// Flight, when its first Bomber stood, and the most it fielded at once.
    flight_observed_turn: Option<u32>,
    advanced_flight_observed_turn: Option<u32>,
    first_bomber_observed_turn: Option<u32>,
    peak_bombers: usize,
    #[serde(skip)]
    observed_actions: usize,
}

impl ConquestProgress {
    fn observe(&mut self, g: &Game) {
        let major = |pid: usize| {
            g.players
                .get(pid)
                .is_some_and(|p| !p.is_minor && !p.is_barbarian)
        };
        if g.players
            .iter()
            .any(|p| p.id != 0 && major(p.id) && g.is_at_war(0, p.id))
        {
            self.first_major_war_observed_turn.get_or_insert(g.turn);
        }
        // Applied declarations distinguish attacking from being attacked and
        // retain wars opened and closed between boundary observations.
        for (seat, action) in g.log.since(self.observed_actions) {
            if *seat != 0 {
                continue;
            }
            let target = match action {
                Action::DeclareWar { player } | Action::DeclareWarWithCasusBelli { player, .. } => {
                    *player
                }
                _ => continue,
            };
            if major(target) {
                self.focal_major_declarations += 1;
                self.first_focal_major_declaration_observed_turn
                    .get_or_insert(g.turn);
                self.first_major_war_observed_turn.get_or_insert(g.turn);
            } else {
                self.focal_minor_declarations += 1;
            }
        }
        self.observed_actions = g.log.len();
        let techs = &g.players[0].techs;
        if techs.iter().any(|tech| tech.as_str() == "flight") {
            self.flight_observed_turn.get_or_insert(g.turn);
        }
        if techs.iter().any(|tech| tech.as_str() == "advanced_flight") {
            self.advanced_flight_observed_turn.get_or_insert(g.turn);
        }
        let bombers = g
            .units
            .values()
            .filter(|unit| {
                unit.owner == 0 && g.rules.units[unit.kind].promotion_class == "air_bomber"
            })
            .count();
        if bombers > 0 {
            self.first_bomber_observed_turn.get_or_insert(g.turn);
        }
        self.peak_bombers = self.peak_bombers.max(bombers);
        self.foreign_capitals_held_at_end = 0;
        self.own_original_capital_held_at_end = false;
        for city in g.cities.values().filter(|city| city.owner == 0) {
            if city.original_owner == 0 {
                self.own_original_capital_held_at_end |= city.is_capital;
            } else if major(city.original_owner) {
                self.first_major_city_held_observed_turn
                    .get_or_insert(g.turn);
                self.foreign_major_cities_observed_held.insert(city.id);
                if city.is_capital {
                    self.first_foreign_capital_held_observed_turn
                        .get_or_insert(g.turn);
                    self.foreign_capitals_observed_held.insert(city.id);
                    self.foreign_capitals_held_at_end += 1;
                }
            }
        }
    }
}

struct Trial {
    outcome: Outcome,
    // Compare the complete canonical applied-action records, not a rounded
    // score or an outcome that could match despite different decisions.
    actions: Vec<u8>,
    civs: Vec<String>,
}

fn trial(seed: u64, difficulty: Option<&str>, policy: &Gene, enabled: bool) -> Trial {
    let mut game = Game::new_with(options(seed, difficulty));
    let civs = game.players.iter().take(4).map(|p| p.civ.clone()).collect();
    let mut ais = fleet(&game, policy, enabled);
    // `CIVVIS_PAIR_EXPLAIN=<dir>` records the focal seat's journal and writes
    // it as `<dir>/<seed>-<on|off>.why.log`, the same lines `--explain` prints.
    let explain_dir = std::env::var_os("CIVVIS_PAIR_EXPLAIN").map(PathBuf::from);
    let journal = explain_dir.as_ref().map(|_| {
        let journal = civvis::reasoning::Journal::recording();
        ais[0].attach_journal(journal.handle());
        journal
    });
    let mut held = BTreeSet::new();
    let mut conquest = ConquestProgress::default();
    let mut economy = Vec::new();
    let mut production_totals = Vec::new();
    let mut accrued_turn = None;
    let mut bankrupt_turns = 0u32;
    let mut bankrupt_seen_turn = None;
    // The journal is a ring of the last few thousand thoughts; drain it at
    // every turn boundary so the file holds the whole game.
    let mut lines = String::new();
    let mut cursor = 0;
    let mut observe = |g: &Game| {
        conquest.observe(g);
        if accrued_turn != Some(g.turn) {
            accrued_turn = Some(g.turn);
            accrue_production(g, &mut production_totals);
        }
        observe_economy(g, &production_totals, &mut economy);
        if g.players[0].bankruptcy_amenity_penalty > 0 && bankrupt_seen_turn != Some(g.turn) {
            bankrupt_seen_turn = Some(g.turn);
            bankrupt_turns += 1;
        }
        for city in g.cities.values() {
            if city.owner == 0 && city.original_owner != 0 {
                held.insert(city.id);
            }
        }
        if let Some(journal) = &journal {
            let delta = journal.since(cursor);
            cursor = delta.cursor;
            for thought in delta.thoughts {
                lines.push_str(&format!(
                    "[why] t{} {:?}/{:?} {} | {}\n",
                    thought.turn, thought.topic, thought.level, thought.headline, thought.detail
                ));
            }
        }
    };
    run_game_observed(&mut game, &mut ais, &mut observe);
    observe(&game);
    if let Some(dir) = explain_dir {
        let path = dir.join(format!("{seed}-{}.why.log", if enabled { "on" } else { "off" }));
        if let Err(error) = std::fs::write(&path, &lines) {
            eprintln!("{}: {error}", path.display());
        }
    }
    let outcome = Outcome {
        winner: game.winner,
        victory: game.victory_type.clone(),
        turn: game.reported_turn(),
        focal_won: game.winner == Some(0),
        focal_domination_won: game.winner == Some(0)
            && game.victory_type.as_deref() == Some("domination"),
        focal_score: game.score(0),
        foreign_cities_observed_held: held.len(),
        foreign_cities_held_at_end: game
            .cities
            .values()
            .filter(|c| c.owner == 0 && c.original_owner != 0)
            .count(),
        applied_actions: game.log.len(),
        conquest,
        air_surge: ais[0].air_surge_census_summary(),
        economy,
        bankrupt_turns,
    };
    Trial {
        outcome,
        actions: serde_json::to_vec(&game.log).expect("applied actions serialize"),
        civs,
    }
}

pub(super) fn run(args: &[String]) -> Result<(), String> {
    let config = parse(args)?;
    for tag in FORCED.trim().split(',') {
        gene(tag).ok_or_else(|| format!("unregistered live bundle policy {tag}"))?;
    }
    let output = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&config.out)
        .map_err(|e| format!("{}: {e}", config.out.display()))?;
    let mut output = BufWriter::new(output);
    let mut changed = 0;
    let mut wins = [0, 0];
    let mut domination = [0, 0];
    for index in 0..config.games {
        let seed = config.start_seed + index;
        // Alternate execution order to avoid confounding every treatment
        // with the second leg. Both worlds and controllers start fresh.
        let (off, on) = if index % 2 == 0 {
            (
                trial(seed, config.difficulty, config.policy, false),
                trial(seed, config.difficulty, config.policy, true),
            )
        } else {
            let on = trial(seed, config.difficulty, config.policy, true);
            (trial(seed, config.difficulty, config.policy, false), on)
        };
        if off.civs != on.civs {
            return Err(format!("paired civilizations differ at seed {seed}"));
        }
        let actions_identical = off.actions == on.actions;
        changed += usize::from(!actions_identical);
        for (i, arm) in [&off, &on].into_iter().enumerate() {
            wins[i] += usize::from(arm.outcome.focal_won);
            domination[i] += usize::from(arm.outcome.focal_domination_won);
        }
        let row = serde_json::json!({
            "schema": 5, "kind": "simulator_domination_policy_pair",
            "policy": config.policy.tag, "seed": seed,
            "execution_order": if index % 2 == 0 { "off,on" } else { "on,off" },
            "profile": profile(seed, config.difficulty), "civilizations": off.civs,
            "focal_seat": 0, "focal_target": "domination",
            "rivals": "adaptive CIVVIS live bridge; not Firaxis AI",
            "forced_focal_policies": FORCED.trim().split(',').collect::<Vec<_>>(),
            "actions_identical": actions_identical,
            "off": off.outcome, "on": on.outcome,
        });
        serde_json::to_writer(&mut output, &row).map_err(|e| e.to_string())?;
        writeln!(output).map_err(|e| e.to_string())?;
        output.flush().map_err(|e| e.to_string())?;
        eprintln!("pair {}/{}, seed {seed}: wins off/on {wins:?}, domination {domination:?}, changed {changed}", index + 1, config.games);
    }
    if changed == 0 {
        return Err(
            "all paired action records are identical; this batch provides no behavioral contrast"
                .to_string(),
        );
    }
    Ok(())
}

#[cfg(test)]
#[path = "domination_pair_tests.rs"]
mod tests;
