//! One production player, with observation and execution owned by adapters.
use std::collections::BTreeSet;
use std::sync::Arc;

use super::{finishing::begin_player_turn, AdvancedAi};
use crate::game::{Action, Game, Item};
use crate::Pos;

pub mod aid;

/// Changes to information access or the fixed policy bundle change the
/// experimental regime even when the set of randomized gene names does not.
pub const CONTRACT: &str = "observed-player-v1";
/// Coverage prior, not a claim to have recovered Firaxis's private policy.
pub const TRAINING_TARGETS: &str = "civvis,science,culture,religion,diplomatic,domination,score";
/// Opening frame plus the live launcher's two default observation refreshes.
pub const REPLAN_FRAMES: usize = 2;

/// The production decision sequence, shared rather than independently
/// orchestrated by the native and live executors. The supplied board must be
/// disposable observed state; the caller owns execution and re-observation.
pub fn plan_frame(
    ai: &mut AdvancedAi,
    view: &mut Game,
    pid: usize,
    mapped: &std::collections::BTreeMap<u32, i64>,
) -> (super::finishing::WarFinishingVolley, usize) {
    let finishing = begin_player_turn(ai, view, pid, mapped);
    let ordinary_begin = view.log.len();
    aid::plan_native(view, pid);
    ai.plan_observed_turn(view, pid);
    (finishing, ordinary_begin)
}

pub fn parse_targets(text: &str) -> Result<Vec<Option<super::VictoryTarget>>, String> {
    if text.is_empty() {
        return Err("target mix must not be empty".into());
    }
    text.split(',')
        .map(|target| {
            if target == "civvis" {
                Ok(None)
            } else {
                target
                    .parse()
                    .map(Some)
                    .map_err(|_| format!("unknown victory target: {target}"))
            }
        })
        .collect()
}

/// Domain-separated from map generation and genome sampling. Repeated entries
/// are explicit weights; no chair is reserved and every genome remains data.
pub fn target_for(
    seed: u64,
    seat: usize,
    targets: &[Option<super::VictoryTarget>],
) -> Option<super::VictoryTarget> {
    assert!(!targets.is_empty());
    let mut mixed = seed ^ (seat as u64).wrapping_mul(0x9e3779b97f4a7c15) ^ 0x504c415945524149;
    mixed = (mixed ^ (mixed >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
    mixed = (mixed ^ (mixed >> 27)).wrapping_mul(0x94d049bb133111eb);
    mixed ^= mixed >> 31;
    targets[(mixed % targets.len() as u64) as usize]
}

/// Authoritative tournament telemetry, never fed back to a player's view.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct PaceSample {
    pub turn: u32,
    /// Older trajectory files did not distinguish elimination from zero pace.
    #[serde(default)]
    pub alive: Option<bool>,
    pub cities: usize,
    pub techs: usize,
    pub civics: usize,
    pub science: f64,
    pub culture: f64,
    pub military: f64,
    pub wars: usize,
}

impl PaceSample {
    pub fn observe(game: &Game, pid: usize) -> Self {
        let cities = game.player_city_ids(pid);
        let mut yields = game.player_yield_extras(pid);
        for id in &cities {
            yields.add(game.city_yields(*id));
        }
        Self {
            turn: game.turn,
            alive: Some(game.players[pid].alive),
            cities: cities.len(),
            techs: game.players[pid].techs.len(),
            civics: game.players[pid].civics.len(),
            science: yields.science,
            culture: yields.culture,
            military: game.military_power(pid),
            wars: (0..game.players.len())
                .filter(|other| {
                    !game.players[*other].is_minor
                        && !game.players[*other].is_barbarian
                        && game.is_at_war(pid, *other)
                })
                .count(),
        }
    }
}

pub fn take_turn(ai: &mut AdvancedAi, game: &mut Game, pid: usize) {
    // Production orders the authoritative board refused this turn. A view
    // can offer what the board will not build — a wonder site on a strategic
    // resource the seat has not revealed yet is one measured case — and
    // without this every replanned frame chose the same refused order again,
    // so the city ended its turn with an empty queue and its Production was
    // lost. Carried as the same per-city block the live bridge fills from
    // host refusals, for this turn's remaining frames only.
    let mut refused: Vec<(u32, Item)> = Vec::new();
    // Re-observe after discovery, combat, or a rejected hypothesis. Never
    // execute the tail of a plan whose actor IDs or tactical facts went stale.
    for _frame in 0..=REPLAN_FRAMES {
        if game.current != pid || game.winner.is_some() {
            return;
        }
        let mut view = game.player_decision_view(pid);
        block_refused_production(&mut view, &refused);
        block_refused_city_sites(&mut view, &ai.refused_city_sites);
        let mapped = view.units.keys().map(|id| (*id, i64::from(*id))).collect();
        let movement_memory = ai.observed_movement_memory();
        let (finishing, ordinary_begin) = plan_frame(ai, &mut view, pid, &mapped);
        // Governor preferences are player decisions, not predicted resources.
        game.players[pid].citizen_food_bias = view.players[pid].citizen_food_bias;
        game.players[pid].city_directives = view.players[pid].city_directives.clone();
        let mut executed_movement = Vec::new();
        let changed = execute_frame_recorded(
            game,
            pid,
            &finishing,
            view.log.since(ordinary_begin),
            &mut executed_movement,
            &mut refused,
            &mut ai.refused_city_sites,
        );
        ai.reconcile_observed_movement(game, movement_memory, &executed_movement);
        if !changed {
            break;
        }
        *game.players[pid]
            .counters
            .entry("player:replans".into())
            .or_default() += 1;
    }
    // A random combat outcome can introduce a mandatory city disposition
    // absent from the projection. Resolve only that explicit blocking choice.
    for _ in 0..64 {
        if game.current != pid || game.winner.is_some() || game.apply(pid, &Action::EndTurn).is_ok()
        {
            break;
        }
        let resolution = game.legal_actions(pid).into_iter().find(|action| {
            matches!(
                action,
                Action::KeepCity { .. } | Action::LiberateCity { .. }
            )
        });
        let Some(action) = resolution else {
            break;
        };
        if game.apply(pid, &action).is_err() {
            break;
        }
    }
}

/// Block every refused production order in a freshly observed view, so the
/// frame planned on it cannot choose the same order again.
fn block_refused_production(view: &mut Game, refused: &[(u32, Item)]) {
    if refused.is_empty() {
        return;
    }
    let mut blocked = (*view.blocked_production).clone();
    for (city, item) in refused {
        blocked
            .entry(*city)
            .or_default()
            .insert(Game::production_block_key(item));
    }
    view.replace_blocked_production(blocked);
}

/// Block every site this seat has been refused a city on, so no frame plans a
/// Settler onto one again. The view is fog-honest and the board is not: a
/// city the seat has never seen, standing within three tiles, makes a site
/// the view offers one the board refuses. Without this the planner re-chose
/// the same site every frame of every turn, and a Settler stood on it for 39
/// turns (ladder proxy, King, seed 37140001: Geneva, unseen, three tiles
/// away). The native bridge already carries host refusals the same way —
/// `refused_sites` into `Game::blocked_city_sites` — so this is that channel,
/// filled from the in-engine board's refusal instead of the host's.
fn block_refused_city_sites(view: &mut Game, refused: &BTreeSet<Pos>) {
    if refused.is_empty() {
        return;
    }
    Arc::make_mut(&mut view.blocked_city_sites).extend(refused.iter().copied());
}

#[cfg(test)]
fn execute_frame<'a>(
    game: &mut Game,
    pid: usize,
    finishing: &super::finishing::WarFinishingVolley,
    ordinary: impl Iterator<Item = &'a (usize, Action)>,
) -> bool {
    execute_frame_recorded(
        game,
        pid,
        finishing,
        ordinary,
        &mut Vec::new(),
        &mut Vec::new(),
        &mut BTreeSet::new(),
    )
}

fn execute_frame_recorded<'a>(
    game: &mut Game,
    pid: usize,
    finishing: &super::finishing::WarFinishingVolley,
    ordinary: impl Iterator<Item = &'a (usize, Action)>,
    movement: &mut Vec<(u32, crate::Pos, crate::Pos)>,
    refused: &mut Vec<(u32, Item)>,
    refused_sites: &mut BTreeSet<Pos>,
) -> bool {
    let mut changed = false;
    for (target, actions) in &finishing.execution {
        if game.units.contains_key(target) {
            for action in actions {
                match execute_observed_action_recorded(
                    game,
                    pid,
                    action,
                    movement,
                    refused,
                    refused_sites,
                ) {
                    Some(refresh) => changed |= refresh,
                    None => return true,
                }
            }
        }
    }
    // Complete the conditional volley as a batch, then observe its actual
    // result before ordinary movement. The frame budget is NOT an attack cap.
    if changed {
        return true;
    }
    for (seat, action) in ordinary {
        if *seat == pid && !matches!(action, Action::EndTurn) {
            match execute_observed_action_recorded(
                game,
                pid,
                action,
                movement,
                refused,
                refused_sites,
            ) {
                Some(refresh) => changed |= refresh,
                None => return true,
            }
        }
    }
    changed
}

/// None stops a tactical refusal or terminal batch immediately. Some(true) requests a
/// fresh observation AFTER the batch, preserving the live adapter's bounded
/// batch cadence instead of silently limiting an army to three attacks.
#[cfg(test)]
fn execute_observed_action(game: &mut Game, pid: usize, action: &Action) -> Option<bool> {
    execute_observed_action_recorded(
        game,
        pid,
        action,
        &mut Vec::new(),
        &mut Vec::new(),
        &mut BTreeSet::new(),
    )
}

fn execute_observed_action_recorded(
    game: &mut Game,
    pid: usize,
    action: &Action,
    movement: &mut Vec<(u32, crate::Pos, crate::Pos)>,
    refused: &mut Vec<(u32, Item)>,
    refused_sites: &mut BTreeSet<Pos>,
) -> Option<bool> {
    if game.current != pid || game.winner.is_some() {
        return None;
    }
    let founding = match action {
        Action::FoundCity { unit } => game.units.get(unit).map(|settler| (*unit, settler.pos)),
        _ => None,
    };
    let explored = game.players[pid].explored.len();
    let allocator = game.next_id;
    let moved_unit = match action {
        Action::Move { unit, .. } | Action::MoveTo { unit, .. } => {
            game.units.get(unit).map(|u| (*unit, u.pos))
        }
        _ => None,
    };
    if game.apply(pid, action).is_err() {
        *game.players[pid]
            .counters
            .entry("player:refused".into())
            .or_default() += 1;
        if let Action::Produce { city, item } = action {
            refused.push((*city, item.clone()));
        }
        // Only the site itself is condemned: the Settler's own tile, read
        // before the order and checked against the board's own founding
        // rule, so a refusal for any other reason (a policy, a unit that is
        // not a Settler) blocks nothing. Like the native bridge's, the block
        // lasts the game.
        if let Some((unit, site)) = founding {
            if game
                .units
                .get(&unit)
                .is_some_and(|settler| settler.pos == site)
                && !game.can_found_city(unit)
                && refused_sites.insert(site)
            {
                *game.players[pid]
                    .counters
                    .entry("player:refused_city_site".into())
                    .or_default() += 1;
            }
        }
        // A rejected queue or financial trade leaves independent military
        // orders executable. Refresh after the batch: stopping here can
        // repeat the same economic refusal in every frame and never reach
        // the army. Orders still pay their authoritative costs when applied.
        // District foundations clear terrain; city transfers and access
        // treaties change tactical facts too. Their refusals still invalidate
        // the remaining plan.
        let economic = match action {
            Action::Produce { item, .. } => !matches!(item, crate::game::Item::District { .. }),
            Action::Trade { offer, request, .. } => {
                offer.cities.is_empty()
                    && request.cities.is_empty()
                    && !offer.open_borders
                    && !request.open_borders
            }
            _ => false,
        };
        return economic.then_some(true);
    }
    if let Some((unit, from)) = moved_unit {
        if let Some(to) = game
            .units
            .get(&unit)
            .map(|u| u.pos)
            .filter(|to| *to != from)
        {
            movement.push((unit, from, to));
        }
    }
    Some(
        game.current != pid
            || game.winner.is_some()
            || game.players[pid].explored.len() != explored
            || game.next_id != allocator
            || matches!(
                action,
                Action::Attack { .. }
                    | Action::Ranged { .. }
                    | Action::CityStrike { .. }
                    | Action::EncampmentStrike { .. }
                    | Action::AirStrike { .. }
                    | Action::TheologicalAttack { .. }
            ),
    )
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod execution_tests;
