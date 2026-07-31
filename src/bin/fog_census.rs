//! Measure whether an AI decision depends on information that fog hides.
//!
//! A normal replay cannot answer that question: if an agent attacks a city it
//! cannot currently see, the replay only shows the action, not whether the
//! agent saw an impermissible fact while choosing it. This census creates a
//! paired information-set test at live decision points instead:
//!
//! 1. choose a hidden enemy unit or city;
//! 2. change its current hit points or relocate a military unit to a legal,
//!    still-hidden neighbouring tile on a reconstructed world;
//! 3. prove the acting seat's [`obs_tensor`] is byte-identical; and
//! 4. replay the exact controller state through one turn on both worlds.
//!
//! An action-trace or observer-plan divergence is therefore a concrete witness
//! that the controller read a fact its tensor did not reveal. A cloned *null*
//! branch is run beside every probe as an integrity control: if an unchanged
//! clone ever makes a different decision, the census refuses to publish a
//! leakage rate.
//!
//! ```text
//! fog_census --maps 12 --probes 12 --players 4 --width 44 --height 28 \
//!   --turns 200 --seed 860000 --jobs 6 --fog-honest-pressure
//! ```
//!
//! This is deliberately a diagnostic, not a new controller. It makes the
//! largest current AI-integrity gap measurable before a fog-honest policy is
//! trusted to close it.

use civvis::ai::{AdvancedAi, Ai, PlanReport};
use civvis::game::{Action, Game};
use civvis::obs::visibility;
use civvis::obs_tensor::{obs_tensor, ObsTensor};
use civvis::{parallel, Pos};
use serde_json::{json, Value};
use std::collections::BTreeSet;

fn number(args: &[String], flag: &str, default: usize) -> usize {
    args.iter()
        .position(|arg| arg == flag)
        .and_then(|index| args.get(index + 1))
        .and_then(|value| value.parse().ok())
        .unwrap_or(default)
}

/// A hidden fact whose current value can be changed on a save/load-rebuilt
/// world. Rebuilding is important for a position treatment: [`Game`] keeps a
/// private occupancy index that must agree with the unit's public position.
/// The tensor equality check is the authoritative guard: a future tensor
/// addition that exposes one of these values makes the probe invalid rather
/// than silently changing what it means.
#[derive(Clone, Copy, Debug)]
enum HiddenFact {
    UnitHp {
        id: u32,
        owner: usize,
        before: i32,
        after: i32,
        at_war: bool,
    },
    CityHp {
        id: u32,
        owner: usize,
        before: i32,
        after: i32,
        at_war: bool,
    },
    UnitPosition {
        id: u32,
        owner: usize,
        from: Pos,
        to: Pos,
        at_war: bool,
    },
}

impl HiddenFact {
    fn kind(self) -> &'static str {
        match self {
            Self::UnitHp { .. } => "unit HP",
            Self::CityHp { .. } => "city HP",
            Self::UnitPosition { .. } => "unit position",
        }
    }

    fn id(self) -> u32 {
        match self {
            Self::UnitHp { id, .. } | Self::CityHp { id, .. } | Self::UnitPosition { id, .. } => id,
        }
    }

    fn owner(self) -> usize {
        match self {
            Self::UnitHp { owner, .. }
            | Self::CityHp { owner, .. }
            | Self::UnitPosition { owner, .. } => owner,
        }
    }

    fn at_war(self) -> bool {
        match self {
            Self::UnitHp { at_war, .. }
            | Self::CityHp { at_war, .. }
            | Self::UnitPosition { at_war, .. } => at_war,
        }
    }

    fn change(self) -> String {
        match self {
            Self::UnitHp { before, after, .. } | Self::CityHp { before, after, .. } => {
                format!("HP {before}→{after}")
            }
            Self::UnitPosition { from, to, .. } => {
                format!("({},{})→({},{})", from.0, from.1, to.0, to.1)
            }
        }
    }

    fn alter_save(self, save: &mut Value) -> bool {
        match self {
            Self::UnitHp { id, after, .. } => {
                replace_save_field(save, "units", id, "hp", json!(after))
            }
            Self::CityHp { id, after, .. } => {
                replace_save_field(save, "cities", id, "hp", json!(after))
            }
            Self::UnitPosition { id, to, .. } => {
                replace_save_field(save, "units", id, "pos", json!([to.0, to.1]))
            }
        }
    }
}

fn alternate_hp(hp: i32) -> i32 {
    if hp > 50 {
        1
    } else {
        100
    }
}

fn enemy_priority(g: &Game, pid: usize, owner: usize, id: u32) -> (bool, bool, usize, u32) {
    (
        !g.is_at_war(pid, owner),
        g.players[owner].is_minor || g.players[owner].is_barbarian,
        owner,
        id,
    )
}

fn hidden_military_unit_ids(g: &Game, pid: usize, visible: &BTreeSet<Pos>) -> Vec<u32> {
    let mut ids: Vec<_> = g
        .units
        .values()
        .filter(|unit| {
            unit.owner != pid
                && unit.hp > 0
                && unit.linked_to.is_none()
                && g.rules
                    .units
                    .get(unit.kind.as_str())
                    .is_some_and(|spec| spec.class == "military" && spec.domain.as_deref() != Some("air"))
                // Position treatments deliberately require both the current
                // and possible destination tile to be fogged. For HP, this
                // predicate still allows a camouflaged unit on a visible tile;
                // the exact tensor check below remains decisive in both cases.
                && !(visible.contains(&unit.pos) && g.unit_visible_to(unit.id, pid))
        })
        .map(|unit| unit.id)
        .collect();
    ids.sort_by_key(|id| {
        let unit = &g.units[id];
        enemy_priority(g, pid, unit.owner, *id)
    });
    ids
}

/// `Game::can_move` accurately checks terrain, borders, stacking, and zones
/// of control, but intentionally includes the transient movement budget. A
/// counterfactual world only needs a position that could have been reached
/// earlier in the unit's turn, so give a scratch clone an ample budget solely
/// for this static-legality check.
fn can_relocate_to(g: &Game, uid: u32, to: Pos) -> bool {
    let mut scratch = g.clone();
    let Some(unit) = scratch.units.get_mut(&uid) else {
        return false;
    };
    unit.moves_left = 100.0;
    unit.attacks_left = 100;
    scratch.can_move(uid, to)
}

fn nearest_owned_city_distance(g: &Game, pid: usize, pos: Pos) -> i32 {
    g.player_city_ids(pid)
        .into_iter()
        .map(|city| g.wdist(g.cities[&city].pos, pos))
        .min()
        .unwrap_or(i32::MAX)
}

fn hidden_unit_position(g: &Game, pid: usize) -> Option<HiddenFact> {
    let (visible, _) = visibility(g, pid);
    let ids = hidden_military_unit_ids(g, pid, &visible);
    if ids.is_empty() {
        return None;
    }
    let mut candidates = Vec::new();
    for id in ids {
        let unit = &g.units[&id];
        if visible.contains(&unit.pos) {
            continue;
        }
        let neighbours: Vec<_> = g.nbrs(unit.pos).into_iter().collect();
        if neighbours.is_empty() {
            continue;
        }
        let neighbour_first = (g.turn as usize + id as usize) % neighbours.len();
        for neighbour_offset in 0..neighbours.len() {
            let to = neighbours[(neighbour_first + neighbour_offset) % neighbours.len()];
            if !visible.contains(&to) && can_relocate_to(g, id, to) {
                let at_war = g.is_at_war(pid, unit.owner);
                let from_distance = nearest_owned_city_distance(g, pid, unit.pos);
                let to_distance = nearest_owned_city_distance(g, pid, to);
                // `AdvancedAi::city_pressure` reads every hostile military
                // unit inside six hexes. A hidden at-war unit crossing this
                // boundary is therefore our highest-value information-set
                // treatment, rather than merely an arbitrary unseen move.
                let pressure_boundary = at_war && from_distance > 6 && to_distance <= 6;
                candidates.push((
                    (!pressure_boundary, !at_war, to_distance, id, to),
                    HiddenFact::UnitPosition {
                        id,
                        owner: unit.owner,
                        from: unit.pos,
                        to,
                        at_war,
                    },
                ));
            }
        }
    }
    candidates.sort_by_key(|(priority, _)| *priority);
    candidates.into_iter().next().map(|(_, fact)| fact)
}

fn hidden_unit_hp(g: &Game, pid: usize) -> Option<HiddenFact> {
    let (visible, _) = visibility(g, pid);
    let mut candidates: Vec<_> = hidden_military_unit_ids(g, pid, &visible)
        .into_iter()
        .map(|id| {
            let unit = &g.units[&id];
            let at_war = g.is_at_war(pid, unit.owner);
            let distance = nearest_owned_city_distance(g, pid, unit.pos);
            let changes_city_pressure = at_war && distance <= 6;
            (
                (!changes_city_pressure, !at_war, distance, id),
                HiddenFact::UnitHp {
                    id,
                    owner: unit.owner,
                    before: unit.hp,
                    after: alternate_hp(unit.hp),
                    at_war,
                },
            )
        })
        .collect();
    candidates.sort_by_key(|(priority, _)| *priority);
    candidates.into_iter().next().map(|(_, fact)| fact)
}

fn hidden_city_hp(g: &Game, pid: usize) -> Option<HiddenFact> {
    let (visible, _) = visibility(g, pid);
    let mut candidates: Vec<_> = g
        .cities
        .values()
        .filter(|city| city.owner != pid && city.hp > 0 && !visible.contains(&city.pos))
        .map(|city| HiddenFact::CityHp {
            id: city.id,
            owner: city.owner,
            before: city.hp,
            after: alternate_hp(city.hp),
            at_war: g.is_at_war(pid, city.owner),
        })
        .collect();
    candidates.sort_by_key(|candidate| enemy_priority(g, pid, candidate.owner(), candidate.id()));
    candidates.into_iter().next()
}

/// Rotate the treatment family, preferring a hidden legal position half the
/// time. The fallback order keeps a sparse early game productive without
/// quietly discarding a requested probe.
fn hidden_fact(g: &Game, pid: usize, probe_index: u64) -> Option<HiddenFact> {
    let position = hidden_unit_position(g, pid);
    let unit_hp = hidden_unit_hp(g, pid);
    let city_hp = hidden_city_hp(g, pid);
    // Use the probe ordinal rather than the game turn: evenly spaced samples
    // can share a turn residue (for example every 16 turns), which otherwise
    // starves two treatment families forever.
    match probe_index % 4 {
        0 | 2 => position.or(unit_hp).or(city_hp),
        1 => unit_hp.or(position).or(city_hp),
        _ => city_hp.or(position).or(unit_hp),
    }
}

fn same_observation(left: &ObsTensor, right: &ObsTensor) -> bool {
    left.width == right.width
        && left.height == right.height
        && left.planes == right.planes
        && left.data == right.data
        && left.global_names == right.global_names
        && left.global == right.global
}

fn replace_save_field(
    save: &mut Value,
    collection: &str,
    id: u32,
    field: &str,
    replacement: Value,
) -> bool {
    let Some(entities) = save.get_mut(collection).and_then(Value::as_array_mut) else {
        return false;
    };
    let Some(entity) = entities.iter_mut().find(|entity| {
        entity
            .get("id")
            .and_then(Value::as_u64)
            .is_some_and(|entity_id| entity_id == u64::from(id))
    }) else {
        return false;
    };
    let Some(object) = entity.as_object_mut() else {
        return false;
    };
    object.insert(field.to_string(), replacement);
    true
}

fn restore_game(save: Value) -> Option<Game> {
    let mut game: Game = serde_json::from_value(save).ok()?;
    // The source world follows headless-simulation semantics. Save/load
    // defaults this presentation cache back on, so restore the source mode
    // before comparing an action trace.
    game.set_fog_memory(false);
    Some(game)
}

/// Build a no-op save/load branch and an otherwise identical treated branch.
/// This prevents a position edit from desynchronizing `Game`'s private
/// occupancy index, and makes save/load itself part of every null control.
fn counterfactual_branches(g: &Game, fact: HiddenFact) -> Option<(Game, Game)> {
    let mut altered_save = serde_json::to_value(g).ok()?;
    let null_game = restore_game(altered_save.clone())?;
    if !fact.alter_save(&mut altered_save) {
        return None;
    }
    let altered_game = restore_game(altered_save)?;
    Some((null_game, altered_game))
}

#[derive(Clone, Debug, PartialEq)]
struct TurnTrace {
    actions: Vec<(usize, Action)>,
    plan: Option<PlanReport>,
}

fn play_turn(ai: &mut AdvancedAi, g: &mut Game, pid: usize) -> TurnTrace {
    let start = g.log.len();
    ai.take_turn(g, pid);
    if g.winner.is_none() && g.current == pid {
        let _ = g.apply(pid, &Action::EndTurn);
    }
    TurnTrace {
        actions: g
            .log
            .since(start)
            .map(|(actor, action)| (*actor, action.clone()))
            .collect(),
        plan: ai.plan_report(),
    }
}

fn first_difference(left: &[(usize, Action)], right: &[(usize, Action)]) -> usize {
    left.iter()
        .zip(right)
        .position(|(left, right)| left != right)
        .unwrap_or_else(|| left.len().min(right.len()))
}

#[derive(Clone, Debug)]
struct Divergence {
    seed: u64,
    turn: u32,
    player: usize,
    fact: HiddenFact,
    plan_changed: bool,
    action_index: Option<usize>,
}

#[derive(Default)]
struct Reading {
    major_turns: u64,
    selected: u64,
    observation_equal: u64,
    observation_mismatches: u64,
    null_observation_matches: u64,
    null_observation_mismatches: u64,
    branch_failures: u64,
    null_matches: u64,
    null_mismatches: u64,
    position_samples: u64,
    unit_samples: u64,
    city_samples: u64,
    wartime_samples: u64,
    divergences: u64,
    plan_divergences: u64,
    action_divergences: u64,
    first_action_divergences: u64,
    examples: Vec<Divergence>,
}

fn is_major(g: &Game, pid: usize) -> bool {
    let player = &g.players[pid];
    !player.is_minor && !player.is_barbarian
}

fn read_map(
    seed: u64,
    players: usize,
    width: i32,
    height: i32,
    turns: u32,
    probes: usize,
    fog_honest_pressure: bool,
) -> Reading {
    let mut game = Game::new(players, width, height, seed, turns, 0);
    // Match ordinary headless simulation: remembered presentation state is not
    // part of decision execution, while explored tiles and live visibility are.
    game.set_fog_memory(false);
    let mut fleet = AdvancedAi::fleet(&game);
    for ai in &mut fleet {
        ai.fog_honest_pressure = fog_honest_pressure;
    }
    let mut reading = Reading::default();
    // A cap alone front-loads every probe into the first few player turns.
    // Spread it through the game so wars, sieges, and mature force planning
    // have a chance to become the counterfactual's decision context.
    let probe_interval = (turns / probes as u32).max(1);
    let mut next_probe_turn = 1;

    while game.winner.is_none() && game.turn <= game.max_turns {
        let pid = game.current;
        let major = is_major(&game, pid);
        reading.major_turns += u64::from(major);
        let can_probe = major && reading.selected < probes as u64 && game.turn >= next_probe_turn;
        let selected = can_probe
            .then(|| hidden_fact(&game, pid, reading.selected))
            .flatten();
        let Some(fact) = selected else {
            let _ = play_turn(&mut fleet[pid], &mut game, pid);
            continue;
        };

        reading.selected += 1;
        next_probe_turn = game.turn.saturating_add(probe_interval);
        match fact {
            HiddenFact::UnitPosition { .. } => reading.position_samples += 1,
            HiddenFact::UnitHp { .. } => reading.unit_samples += 1,
            HiddenFact::CityHp { .. } => reading.city_samples += 1,
        }
        reading.wartime_samples += u64::from(fact.at_war());

        let source_observation = obs_tensor(&game, pid);
        let Some((mut null_game, mut altered_game)) = counterfactual_branches(&game, fact) else {
            reading.branch_failures += 1;
            let _ = play_turn(&mut fleet[pid], &mut game, pid);
            continue;
        };
        let null_observation = obs_tensor(&null_game, pid);
        let altered_observation = obs_tensor(&altered_game, pid);
        let null_observation_equal = same_observation(&source_observation, &null_observation);
        let observation_equal = same_observation(&source_observation, &altered_observation);
        if null_observation_equal {
            reading.null_observation_matches += 1;
        } else {
            reading.null_observation_mismatches += 1;
        }
        if observation_equal {
            reading.observation_equal += 1;
        } else {
            reading.observation_mismatches += 1;
        }

        let mut null_ai = fleet[pid].clone();
        let mut altered_ai = fleet[pid].clone();
        let turn = game.turn;
        let actual_actions = play_turn(&mut fleet[pid], &mut game, pid);
        let null_actions = play_turn(&mut null_ai, &mut null_game, pid);
        if actual_actions == null_actions {
            reading.null_matches += 1;
        } else {
            reading.null_mismatches += 1;
            continue;
        }
        // Both tensor comparisons and the no-op turn are validity conditions
        // for interpreting a changed counterfactual action as information
        // leakage. A malformed or behavior-changing save/load branch makes
        // the whole run fail closed after its remaining maps finish.
        if !null_observation_equal || !observation_equal {
            continue;
        }
        let altered_actions = play_turn(&mut altered_ai, &mut altered_game, pid);
        if actual_actions != altered_actions {
            reading.divergences += 1;
            let plan_changed = actual_actions.plan != altered_actions.plan;
            let actions_changed = actual_actions.actions != altered_actions.actions;
            reading.plan_divergences += u64::from(plan_changed);
            reading.action_divergences += u64::from(actions_changed);
            let action_index = actions_changed
                .then(|| first_difference(&actual_actions.actions, &altered_actions.actions));
            reading.first_action_divergences +=
                u64::from(action_index.is_some_and(|index| index == 0));
            if reading.examples.len() < 3 {
                reading.examples.push(Divergence {
                    seed,
                    turn,
                    player: pid,
                    fact,
                    plan_changed,
                    action_index,
                });
            }
        }
    }
    reading
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let maps = number(&args, "--maps", 12);
    let probes = number(&args, "--probes", 12);
    let players = number(&args, "--players", 4);
    let width = number(&args, "--width", 44) as i32;
    let height = number(&args, "--height", 28) as i32;
    let turns = number(&args, "--turns", 200) as u32;
    let seed0 = number(&args, "--seed", 860_000) as u64;
    let jobs = number(&args, "--jobs", parallel::default_jobs());
    let fog_honest_pressure = args.iter().any(|arg| arg == "--fog-honest-pressure");
    if maps == 0 || probes == 0 || players < 2 || width < 8 || height < 8 || turns == 0 {
        eprintln!(
            "fog_census: maps and probes must be positive; players >= 2; map dimensions >= 8; turns > 0"
        );
        std::process::exit(2);
    }

    println!(
        "fog_census: {maps} maps, {players}p {width}x{height}, cap {turns} turns, \
         {probes} hidden-fact probes/map, seed {seed0}, jobs {jobs}, fog-honest-pressure={fog_honest_pressure}"
    );
    let readings = parallel::map(maps, jobs, move |index| {
        read_map(
            seed0 + index as u64,
            players,
            width,
            height,
            turns,
            probes,
            fog_honest_pressure,
        )
    });

    let mut total = Reading::default();
    for reading in readings {
        total.major_turns += reading.major_turns;
        total.selected += reading.selected;
        total.observation_equal += reading.observation_equal;
        total.observation_mismatches += reading.observation_mismatches;
        total.null_observation_matches += reading.null_observation_matches;
        total.null_observation_mismatches += reading.null_observation_mismatches;
        total.branch_failures += reading.branch_failures;
        total.null_matches += reading.null_matches;
        total.null_mismatches += reading.null_mismatches;
        total.position_samples += reading.position_samples;
        total.unit_samples += reading.unit_samples;
        total.city_samples += reading.city_samples;
        total.wartime_samples += reading.wartime_samples;
        total.divergences += reading.divergences;
        total.plan_divergences += reading.plan_divergences;
        total.action_divergences += reading.action_divergences;
        total.first_action_divergences += reading.first_action_divergences;
        total.examples.extend(reading.examples);
    }

    println!(
        "  major turns reached                 {}",
        total.major_turns
    );
    println!("  hidden facts selected               {}", total.selected);
    println!(
        "  treatment tensor matches           {}/{}",
        total.observation_equal, total.selected
    );
    println!(
        "  save/load tensor matches            {}/{}",
        total.null_observation_matches, total.selected
    );
    println!(
        "  null-control decision matches       {}/{}",
        total.null_matches, total.selected
    );
    println!(
        "  selected facts                      {} unit positions, {} unit HP, {} city HP, {} at war",
        total.position_samples, total.unit_samples, total.city_samples, total.wartime_samples
    );

    if total.observation_mismatches > 0
        || total.null_observation_mismatches > 0
        || total.branch_failures > 0
        || total.null_mismatches > 0
    {
        eprintln!(
            "\nfog_census integrity failure: {} treatment tensor mismatches, {} save/load tensor mismatches, \
             {} branch-construction failures, and {} null-control divergences; \
             refusing to interpret the treatment rate.",
            total.observation_mismatches,
            total.null_observation_mismatches,
            total.branch_failures,
            total.null_mismatches,
        );
        std::process::exit(2);
    }

    if total.selected == 0 {
        println!("\nNo hidden enemy fact was available; raise --maps or --turns.");
        return;
    }
    let rate = 100.0 * total.divergences as f64 / total.selected as f64;
    println!(
        "  decision divergences                 {}/{} ({rate:.1}%)",
        total.divergences, total.selected
    );
    println!(
        "  plan-report divergences              {}/{}",
        total.plan_divergences, total.selected
    );
    println!(
        "  action-trace divergences             {}/{}",
        total.action_divergences, total.selected
    );
    println!(
        "  first-action divergences              {}/{}",
        total.first_action_divergences, total.selected
    );
    for example in total.examples.iter().take(8) {
        let evidence = match (example.plan_changed, example.action_index) {
            (true, Some(index)) => format!("plan changed; first action differs at {index}"),
            (true, None) => "plan changed; actions match this turn".to_string(),
            (false, Some(index)) => format!("first action differs at {index}"),
            (false, None) => "unreachable identical trace".to_string(),
        };
        println!(
            "    seed {} turn {} seat {}: {} {} (owner {}, war={}, {}) — {}",
            example.seed,
            example.turn,
            example.player,
            example.fact.kind(),
            example.fact.id(),
            example.fact.owner(),
            example.fact.at_war(),
            example.fact.change(),
            evidence,
        );
    }
    if total.divergences == 0 {
        println!(
            "\nNo witness in this sample. That does not establish fog honesty: it only says these \
             hidden-fact perturbations did not change an observed plan report or full-turn action trace."
        );
    } else {
        println!(
            "\nEach divergence is a controlled witness: the controller chose differently even \
             though its complete fog-honest tensor was identical."
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hidden_hp_counterfactual_preserves_the_tensor() {
        let mut game = Game::new(4, 40, 24, 86_101, 100, 0);
        game.set_fog_memory(false);
        let (pid, fact) = (0..game.players.len())
            .find_map(|pid| hidden_unit_hp(&game, pid).map(|fact| (pid, fact)))
            .expect("a fresh four-player map has a hidden rival unit or city");
        let before = obs_tensor(&game, pid);
        let (_, altered) = counterfactual_branches(&game, fact).expect("counterfactual branches");
        assert!(matches!(
            fact,
            HiddenFact::UnitHp { before, after, .. } if before != after
        ));
        assert!(same_observation(&before, &obs_tensor(&altered, pid)));
    }

    #[test]
    fn save_load_null_control_repeats_a_full_turn() {
        let mut game = Game::new(2, 24, 16, 86_102, 80, 0);
        game.set_fog_memory(false);
        let pid = game.current;
        let fact = hidden_unit_hp(&game, pid)
            .or_else(|| hidden_city_hp(&game, pid))
            .expect("a fresh two-player map has a hidden rival fact");
        let (mut control_game, _) =
            counterfactual_branches(&game, fact).expect("save/load branches");
        let mut actual_ai = AdvancedAi::new();
        let mut control_ai = actual_ai.clone();
        assert_eq!(
            play_turn(&mut actual_ai, &mut game, pid),
            play_turn(&mut control_ai, &mut control_game, pid),
            "the null control must be exact before an altered branch is meaningful"
        );
    }

    #[test]
    fn hidden_position_rebuilds_occupancy_without_changing_the_tensor() {
        let mut found = None;
        for seed in 86_103..86_123 {
            let mut game = Game::new(4, 40, 24, seed, 100, 0);
            game.set_fog_memory(false);
            for pid in 0..game.players.len() {
                if let Some(fact) = hidden_unit_position(&game, pid) {
                    found = Some((game, pid, fact));
                    break;
                }
            }
            if found.is_some() {
                break;
            }
        }
        let (game, pid, fact) =
            found.expect("a small deterministic seed range has a hidden relocatable rival unit");
        let before = obs_tensor(&game, pid);
        let (_, altered) = counterfactual_branches(&game, fact).expect("counterfactual branches");
        let HiddenFact::UnitPosition { id, to, .. } = fact else {
            panic!("position selector returned the wrong fact family");
        };
        assert_eq!(altered.units[&id].pos, to);
        assert!(altered.units_at(to).contains(&id));
        assert!(same_observation(&before, &obs_tensor(&altered, pid)));
    }

    #[test]
    fn first_difference_handles_a_shorter_trace() {
        let end = (0, Action::EndTurn);
        assert_eq!(first_difference(&[end.clone()], &[]), 0);
        assert_eq!(first_difference(&[end.clone()], &[end]), 1);
    }
}
