//! One production player, with observation and execution owned by adapters.
use std::collections::BTreeSet;
use std::sync::Arc;

use super::{finishing::begin_player_turn, AdvancedAi};
use crate::game::{Action, Game, Item};
use crate::Pos;

pub mod aid;
pub mod native_air_assault;

/// Changes to information access or the fixed policy bundle change the
/// experimental regime even when the set of randomized gene names does not.
pub const CONTRACT: &str = "observed-player-v1";
/// Coverage prior, not a claim to have recovered Firaxis's private policy.
pub const TRAINING_TARGETS: &str = "civvis,science,culture,religion,diplomatic,domination,score";
/// Opening frame plus the live launcher's two default observation refreshes.
pub const REPLAN_FRAMES: usize = 2;

/// How a production player's planning view answers for a city yield it has
/// already derived in the same frame.
///
/// ★★★ THE VIEW WAS THE ONE BOARD THAT KEPT NOTHING. A seat's planning asks
/// what the same cities yield pass after pass, and `Game::keep_rival_yields`
/// with `Game::keep_own_yields` answers the repeats from what the last pass
/// derived until the board changes. The spectator session turned that on for
/// civvis.ai's seats, which plan on the authoritative board. A production
/// player plans on a disposable view instead (`Game::player_decision_view`),
/// the view is a clone, and a clone keeps nothing: so the gene screen, the
/// leader sweeps and every other native game of production players derived
/// every repeat afresh. On the screen's Deity recipe a view answers 47% of
/// its city-yield reads from what it kept, and the game costs 5% fewer
/// instructions (`docs/SIMULATOR_PERFORMANCE.md`, 2026-11-15).
///
/// The contract is `Game::keep_rival_yields`'s and nothing is added to it:
/// whatever is kept is dropped by every action the plan applies to the view,
/// by every tile write, by `Game::board_written`, and for the seat's own
/// cities by any change to its deck, food bias or directives; nothing is
/// reused while a policy trace records. Keeping is switched on after the
/// view is built and blocked, so no yield derived while the view was being
/// corrected is ever kept, and the view is dropped at the end of its frame.
///
/// Only [`take_turn`] does this. The live Civilization VI seat calls
/// [`plan_frame`] on its own mirror board and keeps nothing, as before.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ViewYields {
    /// Kept between the plan's passes. What every production player does.
    #[default]
    Kept,
    /// Derived afresh on every ask, as every view did before. The same game,
    /// the slow way: `gene_screen --no-kept-yields` prices the difference.
    Derived,
    /// Kept, and every reused answer is derived again and compared; a
    /// difference panics (`Game::audit_rival_yields`). Slower than `Derived`.
    Audited,
}

/// One seat's [`ViewYields`] and what its views have answered so far.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ViewYieldKeeping {
    pub mode: ViewYields,
    /// City yields the seat's views answered from what they had kept.
    pub reused: u64,
    /// City yields the seat's views derived while keeping was on.
    pub derived: u64,
}

impl ViewYieldKeeping {
    /// Turn keeping on for a view that is built and about to be planned on.
    fn begin(&self, view: &mut Game) {
        if self.mode == ViewYields::Derived {
            return;
        }
        view.audit_rival_yields(self.mode == ViewYields::Audited);
        view.keep_rival_yields(true);
        view.keep_own_yields(true);
    }

    /// Add what a planned view answered to the seat's running count.
    fn count(&mut self, view: &Game) {
        let (reused, derived) = view.rival_yields_reused();
        self.reused += reused;
        self.derived += derived;
    }
}

impl AdvancedAi {
    /// How this seat's planning views answer for city yields. See
    /// [`ViewYields`]; every mode plays the same game.
    pub fn set_view_yields(&mut self, mode: ViewYields) {
        self.view_yields.mode = mode;
    }

    /// This seat's [`ViewYields`] and its views' running counts.
    pub fn view_yields(&self) -> ViewYieldKeeping {
        self.view_yields
    }
}

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
        // One memo for the whole sweep; see `Game::query_memo`.
        let _memo = game.query_memo();
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
        // From here to the end of the frame only the plan writes the view,
        // and it writes it through the routes that drop a kept yield. See
        // `ViewYields`.
        ai.view_yields.begin(&mut view);
        let (finishing, ordinary_begin) = plan_frame(ai, &mut view, pid, &mapped);
        ai.view_yields.count(&view);
        // Governor preferences are player decisions, not predicted resources.
        game.players[pid].citizen_food_bias = view.players[pid].citizen_food_bias;
        game.players[pid].city_directives = view.players[pid].city_directives.clone();
        game.board_written();
        let mut executed_movement = Vec::new();
        // `districts-price-the-improvement`: like the live bridge, which sends
        // the host only each city's last Produce of a batch, a city's earlier
        // Produce orders this frame are dropped — so a District the planner
        // re-sited before ending its turn never lands on (and bulldozes) the
        // first site. Off, every logged action executes as before.
        let ordinary: Vec<&(usize, Action)> = if ai.districts_price_the_improvement_on() {
            final_production_only(view.log.since(ordinary_begin))
        } else {
            view.log.since(ordinary_begin).collect()
        };
        let changed = execute_frame_recorded(
            game,
            pid,
            &finishing,
            ordinary.into_iter(),
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

/// `districts-price-the-improvement`: drop a city's earlier `Produce` of a
/// District when a later `Produce` this frame re-sites the SAME District
/// elsewhere, as the live bridge (which sends only each city's last Produce)
/// effectively does. Nothing else is dropped: collapsing every earlier order
/// left a city idle whenever the board refused its last one (the war-paced
/// Deity read with the wholesale collapse lost 11.5 Production a turn at t100).
fn final_production_only<'a>(
    actions: impl Iterator<Item = &'a (usize, Action)>,
) -> Vec<&'a (usize, Action)> {
    let actions: Vec<&(usize, Action)> = actions.collect();
    let superseded = |index: usize| -> bool {
        let Action::Produce {
            city,
            item: Item::District { district, pos },
        } = &actions[index].1
        else {
            return false;
        };
        actions[index + 1..].iter().any(|(_, later)| {
            matches!(
                later,
                Action::Produce {
                    city: later_city,
                    item: Item::District { district: later_district, pos: later_pos },
                } if later_city == city && later_district == district && later_pos != pos
            )
        })
    };
    (0..actions.len())
        .filter(|&index| !superseded(index))
        .map(|index| actions[index])
        .collect()
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
        // A rejected queue, plot purchase, proposal or financial trade leaves independent
        // production and military orders executable. Refresh after the batch: stopping here can
        // repeat the same economic refusal in every frame and never reach
        // the army. Orders still pay their authoritative costs when applied.
        // District foundations clear terrain; city transfers and access
        // treaties change tactical facts too. Their refusals still invalidate
        // the remaining plan.
        let economic = match action {
            // A failed annexation changes neither ownership nor treasury.
            // Visible borders can have an unseen city, so the view offers a
            // plot the board refuses; aborting repeats it before any queue.
            Action::BuyPlot { .. } => true,
            // A recruitment the board refuses changes nothing on it. The
            // measured case: `great-person-housing` sells a Great Work on the
            // view to free a slot and recruits the Writer; the board refuses
            // the sale, so the slot stays full and it refuses the recruit.
            // Aborting there repeated both refusals in every frame, and no
            // city with an empty queue received its order.
            Action::RecruitGreatPerson { .. } => true,
            // Proposals are validated before being queued. A refusal cannot
            // change peace, borders or terrain, so it must not starve later
            // Builder work or military orders. Refresh after this batch.
            Action::ProposeDeal { .. } => true,
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
                    | Action::DistrictStrike { .. }
                    | Action::AirStrike { .. }
                    | Action::TheologicalAttack { .. }
            ),
    )
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod execution_tests;

#[cfg(test)]
mod plot_refusal_tests;

#[cfg(test)]
mod kept_yields_tests;
