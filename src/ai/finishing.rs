//! Shared tactical finishing pass used by native and externally executed players.
use crate::game::Action;

/// The same opening tactical policy for every engine adapter. Unit IDs in
/// `mapped` are executable actors, not additional intelligence about enemies.
pub fn begin_player_turn(
    ai: &mut crate::ai::AdvancedAi,
    game: &mut crate::game::Game,
    pid: usize,
    mapped: &std::collections::BTreeMap<u32, i64>,
) -> WarFinishingVolley {
    ai.observe_turn_start_hostiles(game, pid);
    let guards = ai.bound_settler_guards(game, pid);
    finish_live_war_units_excluding(game, pid, mapped, &guards, ai)
}

#[derive(Default)]
pub struct WarFinishingVolley {
    /// Wounded military enemies at war that the exact planning model removed
    /// before ordinary movement.
    pub targets: usize,
    /// Direct attacks to send from the exported frame, including one reserve
    /// attacker when Firaxis may leave a modelled kill alive.
    pub actions: Vec<Action>,
    /// Native execution expands host MOVE_TO into its proven steps and checks
    /// target survival before each primary or conditional reserve line.
    pub execution: Vec<(u32, Vec<Action>)>,
    pub reserves: usize,
    pub survival_rejections: usize,
    pub roll_rejections: usize,
}

#[derive(Clone)]
pub struct FinishingCandidate {
    pub unit: u32,
    /// Exact CIVVIS actions used to prove and score the line.
    pub simulation: Vec<Action>,
    /// One order the host can execute from its exported frame. A melee order
    /// may collapse approach moves plus the final attack into MOVE_TO(enemy).
    pub order: Action,
    pub kills: bool,
    pub ranged: bool,
    pub damage: i32,
    pub attacker_loss: i32,
}

/// Legal attacks a mapped unit can make on `target` from the exported frame.
///
/// Geometry alone is not enough here: line of sight, siege setup, remaining
/// attacks and promotions can all make an apparently direct shot illegal. Each
/// candidate therefore has to survive the engine's own `apply` on a private
/// board before it can pre-empt the unit's ordinary movement order.
pub fn live_finishing_candidates(
    game: &crate::game::Game,
    pid: usize,
    target: u32,
    mapped: &std::collections::BTreeMap<u32, i64>,
    committed: &std::collections::BTreeSet<u32>,
) -> Vec<FinishingCandidate> {
    let Some(defender) = game.units.get(&target) else {
        return Vec::new();
    };
    let target_pos = defender.pos;
    let target_hp = defender.hp;
    let mut candidates = Vec::new();

    for unit in game.player_unit_ids(pid) {
        if committed.contains(&unit) || !mapped.contains_key(&unit) {
            continue;
        }
        let friendly = &game.units[&unit];
        let spec = &game.rules.units[friendly.kind];
        if spec.class != "military"
            || friendly.moves_left <= 0.0
            || friendly.attacks_left <= 0
            || friendly
                .linked_to
                .and_then(|peer| game.units.get(&peer))
                .is_some_and(|peer| game.rules.units[peer.kind].class != "military")
        {
            continue;
        }
        let distance = game.wdist(friendly.pos, target_pos);
        let mut modes = Vec::with_capacity(3);
        if spec.has_ranged_attack() && distance <= game.unit_attack_range(unit) {
            let order = Action::Ranged {
                unit,
                target: target_pos,
            };
            modes.push((true, vec![order.clone()], order));
        }
        if spec.is_melee_capable() && distance == 1 {
            let order = Action::Attack {
                unit,
                target: target_pos,
            };
            modes.push((false, vec![order.clone()], order));
        } else if spec.is_melee_capable() && distance > 1 {
            // Civ VI resolves melee through MOVE_TO on the occupied tile, so
            // one host order can cover an approach and its blow. Prove the
            // corresponding line with CIVVIS's own pathfinder first; a route
            // stopped by terrain, stacking, ZOC or exhausted movement never
            // becomes a host order.
            let mut approach = game.clone();
            let mut simulation = Vec::new();
            for _ in 0..game.map.tiles.len() {
                if approach.wdist(approach.units[&unit].pos, target_pos) <= 1 {
                    break;
                }
                let Some(next) = approach.route_step(unit, target_pos, 1) else {
                    break;
                };
                let step = Action::Move { unit, to: next };
                if approach.apply(pid, &step).is_err() {
                    break;
                }
                simulation.push(step);
            }
            if approach.wdist(approach.units[&unit].pos, target_pos) == 1 {
                let attack = Action::Attack {
                    unit,
                    target: target_pos,
                };
                if approach.apply(pid, &attack).is_ok() {
                    simulation.push(attack);
                    modes.push((
                        false,
                        simulation,
                        Action::Attack {
                            unit,
                            target: target_pos,
                        },
                    ));
                }
            }
        }

        for (ranged, simulation, order) in modes {
            let attacker_hp = friendly.hp;
            let mut after = game.clone();
            let mut legal = true;
            for action in &simulation {
                if after.apply(pid, action).is_err() {
                    legal = false;
                    break;
                }
            }
            if !legal || !after.units.contains_key(&unit) {
                continue;
            }
            let remaining = after.units.get(&target).map(|unit| unit.hp).unwrap_or(0);
            let damage = (target_hp - remaining).max(0);
            if damage == 0 {
                continue;
            }
            let attacker_loss = attacker_hp - after.units[&unit].hp;
            candidates.push(FinishingCandidate {
                unit,
                simulation,
                order,
                kills: !after.units.contains_key(&target),
                ranged,
                damage,
                attacker_loss,
            });
        }
    }

    // A certain kill first; otherwise make the shortest volley. Ranged fire
    // breaks a tie without risking a melee body, then damage and retaliation
    // distinguish otherwise equivalent blows.
    candidates.sort_by(|left, right| {
        right
            .kills
            .cmp(&left.kills)
            .then_with(|| right.ranged.cmp(&left.ranged))
            .then_with(|| right.damage.cmp(&left.damage))
            .then_with(|| left.attacker_loss.cmp(&right.attacker_loss))
            .then_with(|| left.unit.cmp(&right.unit))
    });
    // A unit with both attack modes still contributes only one seat to the
    // volley. The preferred mode is first after the ordering above.
    let mut seen = std::collections::BTreeSet::new();
    candidates.retain(|candidate| seen.insert(candidate.unit));
    candidates
}

/// A lower-roll estimate from the exported board, before any simulated
/// damage weakens the defender. The bridge's RNG is unrelated to Firaxis's:
/// one lucky native kill cannot prove that a live volley finishes its target.
/// This bounds native roll uncertainty, not differences in the host's rules.
pub fn live_finishing_damage_floor(
    game: &crate::game::Game,
    pid: usize,
    target: u32,
    candidate: &FinishingCandidate,
) -> Option<i32> {
    let mut approach = game.clone();
    for action in &candidate.simulation {
        let strengths = match action {
            Action::Move { .. } => {
                approach.apply(pid, action).ok()?;
                continue;
            }
            Action::Attack { unit, .. } => approach.melee_exchange_strengths(*unit, target),
            Action::Ranged {
                unit,
                target: position,
            } => approach.ranged_strike_strengths(*unit, target, *position),
            _ => return None,
        }?;
        // Match `game::damage` at its lowest 0.8 roll, before rounding and
        // clamping. Scaling its already-clamped mean would turn a guaranteed
        // 100-damage blow into an incorrect 80-damage floor.
        let damage = 30.0 * ((strengths.0 - strengths.1) / 25.0).exp() * 0.8;
        return Some((damage.round() as i32).clamp(1, 100));
    }
    None
}

/// Finish exposed wounded enemies before their attackers receive campaign moves.
///
/// The old live-only repair rewrote every wounded barbarian to 100 HP on the
/// planning board. That kept a second defender from being released after a
/// simulated kill, but it also erased the decisive tactical fact: this unit is
/// one blow from death. The active run `civvis-20260815T095258Z` then showed
/// barbarians survive successive exported frames on 1, 3, 6, 8, 16 and 20 HP
/// while nearby units promoted or marched. The reserve was firing throughout.
///
/// Keep the exported HP. For every wounded military unit belonging to a player
/// we are currently at war with, prove a kill using only attacks legal *right
/// now*, apply that shortest volley before the normal AI turn, and reserve one
/// additional direct attacker when one is available. This covers barbarians,
/// rivals, city-states, and Free Cities without treating a visible peacetime
/// unit as a target. Damage-only opportunities remain the normal tactical
/// planner's decision.
///
/// The extra order preserves the host/model safety margin: if Firaxis leaves a
/// predicted kill alive it gets the second blow; if the first blow killed it, a
/// ranged order is refused and a melee order can only enter the cleared tile or
/// be blocked by the first attacker.
pub fn finish_live_war_units(
    planned_game: &mut crate::game::Game,
    pid: usize,
    mapped: &std::collections::BTreeMap<u32, i64>,
) -> WarFinishingVolley {
    finish_live_war_units_excluding(
        planned_game,
        pid,
        mapped,
        &std::collections::BTreeSet::new(),
        &crate::ai::AdvancedAi::new(),
    )
}

/// [`finish_live_war_units`] with `excluded` units left out of the volley
/// entirely — a settler's bound guard under
/// `AdvancedAi::settler_stack_discipline`, which stays out of this pre-pass
/// for the same reason.
pub fn finish_live_war_units_excluding(
    planned_game: &mut crate::game::Game,
    pid: usize,
    mapped: &std::collections::BTreeMap<u32, i64>,
    excluded: &std::collections::BTreeSet<u32>,
    ai: &crate::ai::AdvancedAi,
) -> WarFinishingVolley {
    let withdrawing = ai.live_wounded_unit_reservations(planned_game, pid);
    let mapped: std::collections::BTreeMap<u32, i64> = mapped
        .iter()
        .filter(|(uid, _)| !excluded.contains(uid) && !withdrawing.contains(uid))
        .map(|(uid, civ6)| (*uid, *civ6))
        .collect();
    let mapped = &mapped;
    let mut targets = planned_game
        .units
        .values()
        .filter(|unit| {
            unit.owner != pid
                && planned_game.is_at_war(pid, unit.owner)
                && planned_game.rules.units[unit.kind].class == "military"
                && unit.hp < 100
        })
        .map(|unit| (unit.hp, unit.pos, unit.id))
        .collect::<Vec<_>>();
    targets.sort_unstable();

    let mut result = WarFinishingVolley::default();
    let mut committed = std::collections::BTreeSet::new();
    for (_, _, target) in targets {
        if !planned_game.units.contains_key(&target) {
            continue;
        }
        let initial = live_finishing_candidates(planned_game, pid, target, mapped, &committed);
        if initial.is_empty() {
            continue;
        }

        // Prove that a direct volley actually removes the target before taking
        // any unit away from the ordinary AI. Damage-only opportunities remain
        // the ordinary tactical path's decision.
        let mut proof = planned_game.clone();
        let mut proof_committed = committed.clone();
        let mut chosen = Vec::new();
        while proof.units.contains_key(&target) {
            let Some(candidate) =
                live_finishing_candidates(&proof, pid, target, mapped, &proof_committed)
                    .into_iter()
                    .next()
            else {
                break;
            };
            let mut legal = true;
            for action in &candidate.simulation {
                if proof.apply(pid, action).is_err() {
                    legal = false;
                    break;
                }
            }
            if !legal {
                break;
            }
            proof_committed.insert(candidate.unit);
            chosen.push(candidate);
        }
        if proof.units.contains_key(&target) {
            continue;
        }
        if !ai.live_finishing_actions_survive(
            planned_game,
            pid,
            chosen.iter().flat_map(|candidate| &candidate.simulation),
        ) {
            result.survival_rejections += 1;
            continue;
        }
        // Price each contribution against the original defender's health:
        // sampled earlier damage must not inflate later blows. A reserve can
        // cover a shortfall even when several primary attackers were chosen.
        let primary_floor = chosen.iter().try_fold(0i32, |sum, candidate| {
            Some(sum + live_finishing_damage_floor(planned_game, pid, target, candidate)?)
        });
        let needs_reserve = chosen.len() == 1
            || primary_floor.is_none_or(|damage| damage < planned_game.units[&target].hp);
        // The host issues a reserve only if the target remains alive. Check
        // its survival on the original board where that defender still exists.
        let backup = if needs_reserve {
            initial
                .iter()
                .filter(|candidate| !chosen.iter().any(|primary| primary.unit == candidate.unit))
                .find(|candidate| {
                    let safe =
                        ai.live_finishing_actions_survive(planned_game, pid, &candidate.simulation);
                    if !safe {
                        result.survival_rejections += 1;
                    }
                    safe
                })
        } else {
            None
        };
        let damage_floor = primary_floor.and_then(|sum| match backup {
            Some(candidate) => {
                Some(sum + live_finishing_damage_floor(planned_game, pid, target, candidate)?)
            }
            None => Some(sum),
        });
        if damage_floor.is_none_or(|damage| damage < planned_game.units[&target].hp) {
            result.roll_rejections += 1;
            continue;
        }

        for candidate in &chosen {
            let mut legal = true;
            for action in &candidate.simulation {
                if planned_game.apply(pid, action).is_err() {
                    legal = false;
                    break;
                }
            }
            if !legal {
                break;
            }
            committed.insert(candidate.unit);
            result.actions.push(candidate.order.clone());
            result
                .execution
                .push((target, candidate.simulation.clone()));
        }
        if planned_game.units.contains_key(&target) {
            continue;
        }
        result.targets += 1;

        // Keep the conditional reserve explicit rather than rewriting the
        // target's health or pretending its attack already landed.
        if let Some(backup) = backup {
            if !committed.contains(&backup.unit) {
                if let Some(unit) = planned_game.units.get_mut(&backup.unit) {
                    unit.moves_left = 0.0;
                    unit.attacks_left = 0;
                }
                committed.insert(backup.unit);
                result.actions.push(backup.order.clone());
                result.execution.push((target, backup.simulation.clone()));
                result.reserves += 1;
            }
        }
    }
    result
}
