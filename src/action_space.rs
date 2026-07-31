//! Fixed action encoding for learned-policy experiments.
//!
//! Civ's action space is enormous and variable, so a fixed one-hot head is
//! the wrong shape. Instead every legal action is featurized into a
//! fixed-width vector; a policy scores the candidate set and picks one
//! (pointer-style). That keeps the network's output size constant while the
//! number of legal actions varies from turn to turn. The runtime scripted
//! controllers do not consume this encoding; it currently supports exporters,
//! imitation/ranking experiments, and evaluator arms.
//!
//! `legal_encoded(g, pid)` returns the legal actions alongside their kind
//! indices and feature rows. The kind mask says which of the [`KINDS`]
//! categories are available at all, which is what a hierarchical policy
//! (choose kind, then choose among that kind's actions) needs.
use crate::game::{effective_strength, Action, Game};
use crate::Pos;

/// Every `Action` discriminant, in a stable order. Appending is safe;
/// reordering invalidates trained policies.
pub const KINDS: [&str; 77] = [
    "move", "move_to", "attack", "ranged", "found_city", "improve",
    "found_corporation", "move_product", "contribute_project",
    "contribute_district", "perform_concert", "pillage", "repair_improvement",
    "coastal_raid", "air_rebase", "air_strike", "air_patrol", "produce", "buy",
    "buy_building", "buy_district", "research", "civic", "declare_war",
    "declare_war_with_casus_belli", "make_peace", "denounce", "propose_deal",
    "accept_deal", "reject_deal", "trade", "congress_vote", "assign_spy",
    "spy_mission", "promote_spy", "choose_dedication", "fortify", "upgrade_unit",
    "promote",
    "combine_units", "link_units", "unlink_units", "government", "slot_policy",
    "unslot_policy", "trade_route", "send_envoy", "levy_military",
    "recruit_great_person", "patronize_great_person", "choose_pantheon",
    "choose_secret_society", "assign_governor", "appoint_governor",
    "reassign_governor", "promote_governor", "found_religion", "spread",
    "theological_attack", "condemn_heretic", "heal_religious", "remove_heresy",
    "launch_inquisition", "evangelize_belief", "convert_barbarians",
    "city_strike", "wmd_strike", "encampment_strike", "keep_city", "raze_city",
    "liberate_city", "end_turn", "air_pillage", "priority_target", "upgrade",
    "build_railroad", "buy_plot",
];

/// The original scalar action block. It is kept as an append-only prefix so
/// old and destination-aware representations can be ablated on one corpus.
pub const LEGACY_NUMERIC_WIDTH: usize = 13;

/// Target terrain, role, local-force, and plan-progress terms appended after
/// [`LEGACY_NUMERIC_WIDTH`].
pub const DESTINATION_WIDTH: usize = 35;
/// Actor-role prefix inside the destination block.
pub const DESTINATION_ROLE_WIDTH: usize = 8;
/// Explicit objective terms inside the destination block: present, distance,
/// and progress from the actor's current tile to the candidate.
pub const PLAN_OFFSET: usize = DESTINATION_ROLE_WIDTH;
pub const PLAN_WIDTH: usize = 3;

/// Width of one action's feature row: kind one-hot, the legacy numeric block,
/// then the destination-aware block described in [`features_with_context`].
pub const FEATURE_WIDTH: usize = KINDS.len() + LEGACY_NUMERIC_WIDTH + DESTINATION_WIDTH;

#[derive(Clone, Copy)]
struct SpatialUnit {
    id: u32,
    pos: Pos,
    attack: f64,
    defense: f64,
    range: i32,
    military: bool,
}

/// Facts shared by every candidate in one decision.
///
/// Constructing current visibility and the local force field is deliberately
/// separated from [`features_with_context`]: a decision can contain hundreds
/// of candidates, but those facts change only after one is applied. The
/// explicit objectives are supplied by the caller's high-level plan. Passing
/// an empty slice keeps the generic encoder useful when no planner exists.
pub struct FeatureContext {
    friendly: Vec<SpatialUnit>,
    hostile: Vec<SpatialUnit>,
    own_cities: Vec<Pos>,
    foreign_cities: Vec<Pos>,
    objectives: Vec<Pos>,
}

impl FeatureContext {
    pub fn new(g: &Game, pid: usize, objectives: &[Pos]) -> FeatureContext {
        let visible = g.player_visibility(pid);
        let sample = |unit: &crate::game::Unit| {
            let spec = &g.rules.units[unit.kind];
            let military = spec.class == "military";
            let range = if spec.has_ranged_attack() {
                g.unit_attack_range(unit.id).max(1)
            } else if spec.is_melee_capable() {
                1
            } else {
                0
            };
            SpatialUnit {
                id: unit.id,
                pos: unit.pos,
                attack: effective_strength(g.unit_strength(unit, false), unit.hp),
                defense: effective_strength(g.unit_strength(unit, true), unit.hp),
                range,
                military,
            }
        };
        let friendly = g
            .units
            .values()
            .filter(|unit| unit.owner == pid)
            .map(sample)
            .collect();
        let hostile = g
            .units
            .values()
            .filter(|unit| unit.owner != pid && g.is_at_war(pid, unit.owner))
            .filter(|unit| visible.contains(&unit.pos) && g.unit_visible_to(unit.id, pid))
            .map(sample)
            .collect();
        let own_cities = g
            .cities
            .values()
            .filter(|city| city.owner == pid)
            .map(|city| city.pos)
            .collect();
        // City positions do not move. Once explored, using the position does
        // not reveal its current HP, ownership fabric, or production through
        // fog; target-specific live facts below are read only for legal
        // actions, whose targets are visible.
        let foreign_cities = g
            .cities
            .values()
            .filter(|city| city.owner != pid && g.players[pid].explored.contains(&city.pos))
            .map(|city| city.pos)
            .collect();
        FeatureContext {
            friendly,
            hostile,
            own_cities,
            foreign_cities,
            objectives: objectives
                .iter()
                .copied()
                .filter(|pos| g.map.get(*pos).is_some())
                .collect(),
        }
    }
}

pub fn kind_index(action: &Action) -> usize {
    let name = kind_name(action);
    KINDS
        .iter()
        .position(|k| *k == name)
        .expect("every Action variant is listed in KINDS")
}

pub fn kind_name(action: &Action) -> &'static str {
    match action {
        Action::Move { .. } => "move",
        Action::MoveTo { .. } => "move_to",
        Action::Attack { .. } => "attack",
        Action::Ranged { .. } => "ranged",
        Action::FoundCity { .. } => "found_city",
        Action::Improve { .. } => "improve",
        Action::FoundCorporation { .. } => "found_corporation",
        Action::MoveProduct { .. } => "move_product",
        Action::ContributeProject { .. } => "contribute_project",
        Action::ContributeDistrict { .. } => "contribute_district",
        Action::PerformConcert { .. } => "perform_concert",
        Action::Pillage { .. } => "pillage",
        Action::RepairImprovement { .. } => "repair_improvement",
        Action::CoastalRaid { .. } => "coastal_raid",
        Action::AirRebase { .. } => "air_rebase",
        Action::AirStrike { .. } => "air_strike",
        Action::AirPatrol { .. } => "air_patrol",
        Action::Produce { .. } => "produce",
        Action::Buy { .. } => "buy",
        Action::BuyBuilding { .. } => "buy_building",
        Action::BuyDistrict { .. } => "buy_district",
        Action::BuyPlot { .. } => "buy_plot",
        Action::Research { .. } => "research",
        Action::Civic { .. } => "civic",
        Action::DeclareWar { .. } => "declare_war",
        Action::DeclareWarWithCasusBelli { .. } => "declare_war_with_casus_belli",
        Action::MakePeace { .. } => "make_peace",
        Action::Denounce { .. } => "denounce",
        Action::ProposeDeal { .. } => "propose_deal",
        Action::AcceptDeal { .. } => "accept_deal",
        Action::RejectDeal { .. } => "reject_deal",
        Action::Trade { .. } => "trade",
        Action::CongressVote { .. } => "congress_vote",
        Action::AssignSpy { .. } => "assign_spy",
        Action::SpyMission { .. } => "spy_mission",
        Action::PromoteSpy { .. } => "promote_spy",
        Action::ChooseDedication { .. } => "choose_dedication",
        Action::Fortify { .. } => "fortify",
        Action::UpgradeUnit { .. } => "upgrade_unit",
        Action::Promote { .. } => "promote",
        Action::CombineUnits { .. } => "combine_units",
        Action::LinkUnits { .. } => "link_units",
        Action::UnlinkUnits { .. } => "unlink_units",
        Action::Government { .. } => "government",
        Action::SlotPolicy { .. } => "slot_policy",
        Action::UnslotPolicy { .. } => "unslot_policy",
        Action::TradeRoute { .. } => "trade_route",
        Action::SendEnvoy { .. } => "send_envoy",
        Action::LevyMilitary { .. } => "levy_military",
        Action::RecruitGreatPerson { .. } => "recruit_great_person",
        Action::PatronizeGreatPerson { .. } => "patronize_great_person",
        Action::ChoosePantheon { .. } => "choose_pantheon",
        Action::ChooseSecretSociety { .. } => "choose_secret_society",
        Action::AssignGovernor { .. } => "assign_governor",
        Action::AppointGovernor { .. } => "appoint_governor",
        Action::ReassignGovernor { .. } => "reassign_governor",
        Action::PromoteGovernor { .. } => "promote_governor",
        Action::FoundReligion { .. } => "found_religion",
        Action::Spread { .. } => "spread",
        Action::TheologicalAttack { .. } => "theological_attack",
        Action::CondemnHeretic { .. } => "condemn_heretic",
        Action::HealReligious { .. } => "heal_religious",
        Action::RemoveHeresy { .. } => "remove_heresy",
        Action::LaunchInquisition { .. } => "launch_inquisition",
        Action::EvangelizeBelief { .. } => "evangelize_belief",
        Action::ConvertBarbarians { .. } => "convert_barbarians",
        Action::CityStrike { .. } => "city_strike",
        Action::WmdStrike { .. } => "wmd_strike",
        Action::EncampmentStrike { .. } => "encampment_strike",
        Action::KeepCity { .. } => "keep_city",
        Action::RazeCity { .. } => "raze_city",
        Action::LiberateCity { .. } => "liberate_city",
        Action::EndTurn => "end_turn",
        Action::AirPillage { .. } => "air_pillage",
        Action::PriorityTarget { .. } => "priority_target",
        Action::Upgrade { .. } => "upgrade",
        Action::BuildRailroad { .. } => "build_railroad",
    }
}

/// The tile an action points at, when it has one. Policies use this to look
/// up the corresponding cell of the spatial observation.
pub fn target_tile(g: &Game, action: &Action) -> Option<Pos> {
    match action {
        Action::Move { to, .. } | Action::MoveTo { to, .. } => Some(*to),
        Action::AirRebase { to, .. } | Action::AirPatrol { to, .. } => Some(*to),
        Action::Attack { target, .. }
        | Action::Ranged { target, .. }
        | Action::AirStrike { target, .. }
        | Action::AirPillage { target, .. }
        | Action::PriorityTarget { target, .. }
        | Action::CityStrike { target, .. }
        | Action::WmdStrike { target, .. }
        | Action::EncampmentStrike { target, .. } => Some(*target),
        Action::FoundCity { unit }
        | Action::Improve { unit, .. }
        | Action::Pillage { unit }
        | Action::RepairImprovement { unit }
        | Action::CoastalRaid { unit, .. }
        | Action::Fortify { unit }
        | Action::UpgradeUnit { unit }
        | Action::Promote { unit, .. }
        | Action::Spread { unit }
        | Action::BuildRailroad { unit }
        | Action::PerformConcert { unit } => g.units.get(unit).map(|u| u.pos),
        Action::Upgrade { unit, .. } => g.units.get(unit).map(|u| u.pos),
        Action::Produce { city, .. }
        | Action::Buy { city, .. }
        | Action::BuyBuilding { city, .. }
        | Action::BuyDistrict { city, .. }
        | Action::KeepCity { city }
        | Action::RazeCity { city }
        | Action::LiberateCity { city } => g.cities.get(city).map(|c| c.pos),
        Action::BuyPlot { pos, .. } => Some(*pos),
        _ => None,
    }
}

/// One action's fixed-width feature row without an external strategic plan.
pub fn features(g: &Game, pid: usize, action: &Action) -> Vec<f32> {
    let context = FeatureContext::new(g, pid, &[]);
    features_with_context(g, pid, action, &context)
}

fn nearest(g: &Game, from: Pos, positions: impl Iterator<Item = Pos>) -> Option<i32> {
    positions.map(|position| g.wdist(from, position)).min()
}

fn distance_feature(distance: Option<i32>, scale: f32) -> f32 {
    distance
        .map(|distance| (distance as f32 / scale).clamp(0.0, 1.0))
        .unwrap_or(1.0)
}

fn progress_feature(before: Option<i32>, after: Option<i32>, scale: f32) -> f32 {
    match (before, after) {
        (Some(before), Some(after)) => ((before - after) as f32 / scale).clamp(-1.0, 1.0),
        _ => 0.0,
    }
}

/// Encode one candidate against facts shared by its decision.
///
/// The legacy thirteen values remain an exact prefix. The appended block gives
/// a ranker the causal quantities that differ between two destinations for the
/// same unit: what the unit is for, terrain cost and defense, nearby support,
/// hostile attack coverage, target hardness, and progress toward an explicit
/// high-level objective. Absolute coordinates and movement direction are
/// intentionally absent; learning deterministic tie-breaking would raise
/// imitation accuracy without teaching strategy.
pub fn features_with_context(
    g: &Game,
    pid: usize,
    action: &Action,
    context: &FeatureContext,
) -> Vec<f32> {
    let mut row = vec![0.0f32; FEATURE_WIDTH];
    row[kind_index(action)] = 1.0;
    let base = KINDS.len();
    let tile = target_tile(g, action);
    if let Some(pos) = tile {
        row[base] = 1.0;
        if let Some(t) = g.map.get(pos) {
            let owned = t
                .owner_city
                .and_then(|c| g.cities.get(&c))
                .map(|c| c.owner == pid);
            row[base + 1] = matches!(owned, Some(true)) as u8 as f32;
            row[base + 2] = matches!(owned, Some(false)) as u8 as f32;
        }
        if let Some(cid) = g.city_at(pos) {
            let city = &g.cities[&cid];
            row[base + 3] = (city.hp as f32 / 200.0).clamp(0.0, 1.0);
            row[base + 4] = (city.owner == pid) as u8 as f32;
        }
        let enemy = g.units_at(pos).into_iter().any(|uid| {
            let u = &g.units[&uid];
            u.owner != pid && g.is_at_war(pid, u.owner)
        });
        row[base + 5] = enemy as u8 as f32;
    }
    if let Some(uid) = acting_unit(action) {
        if let Some(unit) = g.units.get(&uid) {
            row[base + 6] = (unit.hp as f32 / 100.0).clamp(0.0, 1.0);
            row[base + 7] =
                (g.unit_strength(unit, false) as f32 / 100.0).clamp(0.0, 1.0);
            row[base + 8] = (unit.moves_left as f32 / 6.0).clamp(0.0, 1.0);
            if let Some(pos) = tile {
                row[base + 9] = (g.wdist(unit.pos, pos) as f32 / 10.0).clamp(0.0, 1.0);
            }
        }
    }
    if let Action::BuyPlot { city, pos, cost } = action {
        if let Some(city) = g.cities.get(city) {
            row[base + 9] = (g.wdist(city.pos, *pos) as f32 / 10.0).clamp(0.0, 1.0);
        }
        row[base + 12] = (*cost as f32 / 2_000.0).clamp(0.0, 1.0);
    }
    // Treasury and Faith give the policy the context that makes purchase
    // actions comparable across turns.
    row[base + 10] = (g.players[pid].gold as f32 / 2000.0).clamp(0.0, 1.0);
    row[base + 11] = (g.players[pid].faith as f32 / 1000.0).clamp(0.0, 1.0);

    let actor = acting_unit(action).and_then(|uid| g.units.get(&uid));
    let actor_spec = actor.map(|unit| &g.rules.units[unit.kind]);
    let actor_uid = actor.map(|unit| unit.id);
    let actor_pos = actor.map(|unit| unit.pos);
    let mut destination = Vec::with_capacity(DESTINATION_WIDTH);

    // Role context is constant inside a same-actor decision, but lets the MLP
    // interpret progress, exposure, and spacing differently for a scout,
    // ranged unit, civilian, or siege train.
    let military = actor_spec.is_some_and(|spec| spec.class == "military");
    let recon = actor_spec.is_some_and(|spec| spec.promotion_class == "recon");
    let mobile = actor_spec.is_some_and(|spec| {
        matches!(
            spec.promotion_class.as_str(),
            "light_cavalry" | "naval_raider" | "naval_melee"
        )
    });
    let ranged = actor_spec.is_some_and(|spec| spec.has_ranged_attack() && !spec.siege);
    let siege = actor_spec.is_some_and(|spec| spec.siege);
    let support = actor_spec.is_some_and(|spec| {
        spec.class == "support"
            || spec.class == "military"
                && !spec.is_melee_capable()
                && !spec.has_ranged_attack()
    });
    let religious = actor_spec.is_some_and(|spec| spec.class == "religious");
    let civilian = actor_spec.is_some_and(|spec| {
        !matches!(spec.class.as_str(), "military" | "support" | "religious")
    });
    for flag in [
        military, recon, mobile, ranged, siege, support, religious, civilian,
    ] {
        destination.push(flag as u8 as f32);
    }

    let objective = actor_pos
        .and_then(|from| {
            context
                .objectives
                .iter()
                .copied()
                .min_by_key(|position| (g.wdist(from, *position), *position))
        })
        .or_else(|| context.objectives.first().copied());
    let objective_before = actor_pos.zip(objective).map(|(from, goal)| g.wdist(from, goal));
    let objective_after = tile.zip(objective).map(|(to, goal)| g.wdist(to, goal));
    destination.push(objective.is_some() as u8 as f32);
    destination.push(distance_feature(objective_after, 20.0));
    destination.push(progress_feature(objective_before, objective_after, 4.0));

    let mut move_cost = 0.0f32;
    let mut defense = 0.0f32;
    let mut water = 0.0f32;
    let mut road = 0.0f32;
    let mut developed = 0.0f32;
    let mut yields = 0.0f32;
    if let Some((pos, target)) = tile.and_then(|pos| g.map.get(pos).map(|target| (pos, target))) {
        let cost = actor_pos
            .filter(|from| g.wdist(*from, pos) == 1)
            .map(|from| g.step_cost(from, pos))
            .unwrap_or_else(|| g.rules.move_cost(target));
        move_cost = (cost as f32 / 4.0).clamp(0.0, 1.0);
        defense = ((if target.hills { 3.0 } else { 0.0 })
            + target
                .feature
                .as_deref()
                .and_then(|feature| g.rules.features.get(feature))
                .map_or(0.0, |feature| feature.defense)) as f32
            / 10.0;
        defense = defense.clamp(-1.0, 1.0);
        water = g.rules.is_water(target) as u8 as f32;
        road = target.road as f32 / 5.0;
        developed = (target.improvement.is_some() || target.district.is_some()) as u8 as f32;
        let mut known = target.clone();
        if known
            .resource
            .as_deref()
            .is_some_and(|resource| !g.resource_visible_to(pid, resource))
        {
            known.resource = None;
        }
        yields = (g.rules.tile_yields(&known).total() as f32 / 10.0).clamp(0.0, 1.0);
    }
    destination.extend([move_cost, defense, water, road, developed, yields]);

    let friends = |position: Pos| {
        context
            .friendly
            .iter()
            .filter(|friend| Some(friend.id) != actor_uid)
            .map(|friend| g.wdist(position, friend.pos))
            .min()
    };
    let hostile = |position: Pos| nearest(g, position, context.hostile.iter().map(|unit| unit.pos));
    let foreign = |position: Pos| nearest(g, position, context.foreign_cities.iter().copied());
    let home = |position: Pos| nearest(g, position, context.own_cities.iter().copied());
    let friend_before = actor_pos.and_then(friends);
    let friend_after = tile.and_then(friends);
    destination.push(distance_feature(friend_after, 10.0));
    destination.push(progress_feature(friend_before, friend_after, 4.0));

    let (adjacent_friends, friendly_strength) = tile.map_or((0usize, 0.0f64), |position| {
        context
            .friendly
            .iter()
            .filter(|friend| Some(friend.id) != actor_uid && friend.military)
            .filter(|friend| g.wdist(position, friend.pos) <= 1)
            .fold((0usize, 0.0f64), |(count, strength), friend| {
                (count + 1, strength + friend.defense)
            })
    });
    destination.push((adjacent_friends as f32 / 6.0).clamp(0.0, 1.0));
    destination.push((friendly_strength as f32 / 300.0).clamp(0.0, 1.0));

    let hostile_before = actor_pos.and_then(hostile);
    let hostile_after = tile.and_then(hostile);
    destination.push(distance_feature(hostile_after, 12.0));
    destination.push(progress_feature(hostile_before, hostile_after, 4.0));
    let (coverage, threat) = tile.map_or((0usize, 0.0f64), |position| {
        context
            .hostile
            .iter()
            .filter(|enemy| enemy.military && enemy.range > 0)
            .filter(|enemy| g.wdist(position, enemy.pos) <= enemy.range)
            .fold((0usize, 0.0f64), |(count, strength), enemy| {
                (count + 1, strength + enemy.attack)
            })
    });
    destination.push((coverage as f32 / 6.0).clamp(0.0, 1.0));
    destination.push((threat as f32 / 300.0).clamp(0.0, 1.0));

    let foreign_before = actor_pos.and_then(foreign);
    let foreign_after = tile.and_then(foreign);
    destination.push(distance_feature(foreign_after, 20.0));
    destination.push(progress_feature(foreign_before, foreign_after, 4.0));
    let home_before = actor_pos.and_then(home);
    let home_after = tile.and_then(home);
    destination.push(distance_feature(home_after, 20.0));
    destination.push(progress_feature(home_before, home_after, 4.0));

    let neighbors = tile.map(|position| g.nbrs(position)).unwrap_or_default();
    let neighbor_count = neighbors.len().max(1) as f32;
    let frontier = neighbors
        .iter()
        .filter(|position| !g.players[pid].explored.contains(position))
        .count() as f32
        / neighbor_count;
    let exits = neighbors
        .iter()
        .filter(|position| g.map.get(**position).is_some_and(|cell| g.rules.is_passable(cell)))
        .count() as f32
        / neighbor_count;
    destination.extend([frontier, exits]);

    let defender = tile.and_then(|position| {
        context
            .hostile
            .iter()
            .filter(|enemy| enemy.pos == position)
            .max_by(|left, right| left.defense.total_cmp(&right.defense))
    });
    let defender_hp = defender
        .and_then(|enemy| g.units.get(&enemy.id))
        .map_or(0.0, |unit| (unit.hp as f32 / 100.0).clamp(0.0, 1.0));
    let mut defender_strength = defender.map_or(0.0, |enemy| enemy.defense);
    if let Some(city) = tile
        .and_then(|position| g.city_at(position))
        .filter(|city| g.cities[city].owner != pid && g.is_at_war(pid, g.cities[city].owner))
    {
        defender_strength = defender_strength.max(g.city_strength(city));
    }
    let attacker_strength = actor.map_or(0.0, |unit| {
        effective_strength(g.unit_strength(unit, false), unit.hp)
    });
    destination.push(defender_hp);
    destination.push((defender_strength as f32 / 100.0).clamp(0.0, 1.5));
    destination.push(
        ((attacker_strength - defender_strength) as f32 / 100.0).clamp(-1.0, 1.0),
    );
    let support = tile.map_or(0usize, |position| {
        context
            .friendly
            .iter()
            .filter(|friend| Some(friend.id) != actor_uid && friend.military && friend.range > 0)
            .filter(|friend| g.wdist(position, friend.pos) <= friend.range)
            .count()
    });
    destination.push((support as f32 / 6.0).clamp(0.0, 1.0));

    debug_assert_eq!(destination.len(), DESTINATION_WIDTH);
    row[base + LEGACY_NUMERIC_WIDTH..].copy_from_slice(&destination);
    row
}

/// Unit whose activation an action consumes, when the action belongs to one.
/// Exposed so policy datasets can distinguish alternative destinations for the
/// same unit from the separate decision of which unit to activate next.
pub fn acting_unit(action: &Action) -> Option<u32> {
    match action {
        Action::Move { unit, .. }
        | Action::MoveTo { unit, .. }
        | Action::Attack { unit, .. }
        | Action::Ranged { unit, .. }
        | Action::FoundCity { unit }
        | Action::Improve { unit, .. }
        | Action::Pillage { unit }
        | Action::RepairImprovement { unit }
        | Action::CoastalRaid { unit, .. }
        | Action::AirRebase { unit, .. }
        | Action::AirStrike { unit, .. }
        | Action::AirPatrol { unit, .. }
        | Action::AirPillage { unit, .. }
        | Action::PriorityTarget { unit, .. }
        | Action::Upgrade { unit, .. }
        | Action::Fortify { unit }
        | Action::UpgradeUnit { unit }
        | Action::Promote { unit, .. }
        | Action::Spread { unit }
        | Action::PerformConcert { unit } => Some(*unit),
        _ => None,
    }
}

pub struct Encoded {
    pub actions: Vec<Action>,
    pub kinds: Vec<usize>,
    /// `actions.len() * FEATURE_WIDTH`, row-major.
    pub features: Vec<f32>,
    /// Which of [`KINDS`] appear in this legal set.
    pub kind_mask: [bool; KINDS.len()],
}

pub fn legal_encoded(g: &Game, pid: usize) -> Encoded {
    legal_encoded_with_objectives(g, pid, &[])
}

/// Encode the legal set with objectives from the caller's high-level plan.
/// The same context is reused for the whole set, so spatial observation cost
/// is per decision rather than per candidate.
pub fn legal_encoded_with_objectives(g: &Game, pid: usize, objectives: &[Pos]) -> Encoded {
    let actions = g.legal_actions(pid);
    let mut kinds = Vec::with_capacity(actions.len());
    let mut features = Vec::with_capacity(actions.len() * FEATURE_WIDTH);
    let mut kind_mask = [false; KINDS.len()];
    let context = FeatureContext::new(g, pid, objectives);
    for action in &actions {
        let k = kind_index(action);
        kinds.push(k);
        kind_mask[k] = true;
        features.extend(features_with_context(g, pid, action, &context));
    }
    Encoded {
        actions,
        kinds,
        features,
        kind_mask,
    }
}

#[cfg(test)]
mod tests {
    use super::{
        features_with_context, kind_index, legal_encoded, FeatureContext, FEATURE_WIDTH,
        KINDS, LEGACY_NUMERIC_WIDTH,
    };
    use crate::ai::{Ai, AdvancedAi};
    use crate::game::{Action, Game};

    #[test]
    fn kinds_are_unique_and_cover_every_legal_action() {
        let mut sorted = KINDS.to_vec();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(sorted.len(), KINDS.len(), "duplicate kind name");

        // Play a real game and encode every legal action seen along the way:
        // any unlisted variant panics in kind_index.
        let mut g = Game::new(4, 28, 18, 12, 60, 2);
        let mut ais = AdvancedAi::fleet(&g);
        let mut seen = std::collections::BTreeSet::new();
        for _ in 0..40 {
            if g.winner.is_some() {
                break;
            }
            let pid = g.current;
            for action in g.legal_actions(pid) {
                seen.insert(kind_index(&action));
            }
            ais[pid].take_turn(&mut g, pid);
            if g.winner.is_none() && g.current == pid {
                let _ = g.apply(pid, &Action::EndTurn);
            }
        }
        assert!(seen.len() > 5, "expected a varied action set, saw {seen:?}");
    }

    #[test]
    fn encoding_shape_matches_the_legal_set() {
        let g = Game::new(4, 28, 18, 4, 80, 2);
        let e = legal_encoded(&g, 0);
        assert!(!e.actions.is_empty());
        assert_eq!(e.kinds.len(), e.actions.len());
        assert_eq!(e.features.len(), e.actions.len() * FEATURE_WIDTH);
        assert!(e.features.iter().all(|v| v.is_finite()));
        // The mask must agree with the encoded kinds exactly.
        for (index, present) in e.kind_mask.iter().enumerate() {
            assert_eq!(*present, e.kinds.contains(&index), "mask disagrees at {index}");
        }
        // Each row's one-hot names that row's kind.
        for (row, kind) in e.kinds.iter().enumerate() {
            let slice = &e.features[row * FEATURE_WIDTH..(row + 1) * FEATURE_WIDTH];
            assert_eq!(slice[*kind], 1.0);
            assert_eq!(slice[..KINDS.len()].iter().sum::<f32>(), 1.0);
        }
    }

    #[test]
    fn an_explicit_objective_distinguishes_destinations_for_one_unit() {
        let g = Game::new(4, 28, 18, 41, 80, 2);
        let moves: Vec<(u32, crate::Pos)> = g
            .legal_actions(0)
            .into_iter()
            .filter_map(|action| match action {
                Action::Move { unit, to } => Some((unit, to)),
                _ => None,
            })
            .collect();
        let pair = moves
            .windows(2)
            .find(|pair| pair[0].0 == pair[1].0)
            .expect("an opening unit should have two legal destinations");
        let objective = pair[0].1;
        let first = Action::Move {
            unit: pair[0].0,
            to: pair[0].1,
        };
        let second = Action::Move {
            unit: pair[1].0,
            to: pair[1].1,
        };
        let context = FeatureContext::new(&g, 0, &[objective]);
        let first = features_with_context(&g, 0, &first, &context);
        let second = features_with_context(&g, 0, &second, &context);
        let destination = KINDS.len() + LEGACY_NUMERIC_WIDTH;
        assert_eq!(first[destination + 8], 1.0, "objective-present flag");
        assert_eq!(first[destination + 9], 0.0, "the objective itself is zero away");
        assert!(second[destination + 9] > 0.0, "the other destination is farther away");
        assert!(first[destination + 10] > second[destination + 10]);
    }
}
