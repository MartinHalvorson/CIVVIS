//! Forcing replies: pricing the enemy's strongest answer to an attack.
//!
//! Before a unit attacks, the tactical search asks what the enemy can do to it
//! afterwards. `forcing_attacks_to` builds the small set of attacks on one
//! victim, `forcing_reply_line` searches them to `FORCING_REPLY_DEPTH`, keeping
//! the `FORCING_REPLY_WIDTH` strongest, and `forcing_reply_penalty` and its
//! `_applied` and `_from_position` forms price the result on the board at hand.
//! `enemy_can_force_a_reply_against_any` answers whether a search is needed at
//! all, and `note_illegal_attack` records a candidate the engine refused.
//!
//! Moved verbatim out of the `impl AdvancedAi` block in `advanced.rs`
//! (2026-11-06). Methods the rest of the controller calls are `pub(super)`; the
//! code is otherwise unchanged.

use super::*;

impl AdvancedAi {
    /// Candidate attacks on one tactical victim. `Game::legal_actions` also
    /// enumerates production, diplomacy, spies, purchases, every movement,
    /// and every other unit target; calling it at each reply-search node made
    /// late-game turns spend most of their time proving irrelevant actions
    /// irrelevant. Build the small target-specific superset here and let
    /// `Game::apply` remain the authoritative legality check.
    pub(super) fn forcing_attacks_to(
        position: &Game,
        enemy: usize,
        victim_pos: Pos,
        only_unit: Option<u32>,
    ) -> Vec<Action> {
        let mut replies = Vec::new();
        for unit in position.units.values().filter(|unit| {
            unit.owner == enemy && only_unit.is_none_or(|candidate| candidate == unit.id)
        }) {
            if unit.moves_left <= 0.0 || unit.attacks_left <= 0 {
                continue;
            }
            let spec = &position.rules.units[unit.kind];
            let distance = position.wdist(unit.pos, victim_pos);
            if spec.domain.as_deref() == Some("air") {
                if distance <= position.unit_attack_range(unit.id) {
                    replies.push(Action::AirStrike {
                        unit: unit.id,
                        target: victim_pos,
                    });
                }
                continue;
            }
            if spec.class != "military" || position.is_embarked(unit) {
                continue;
            }
            if spec.has_ranged_attack() && distance <= position.unit_attack_range(unit.id) {
                replies.push(Action::Ranged {
                    unit: unit.id,
                    target: victim_pos,
                });
            }
            if spec.is_melee_capable() && distance == 1 {
                replies.push(Action::Attack {
                    unit: unit.id,
                    target: victim_pos,
                });
            }
        }
        if only_unit.is_none() {
            for city in position.cities.values().filter(|city| city.owner == enemy) {
                // ★★★★ THE CALLER PROVES THESE ILLEGAL BY CLONING THE WHOLE
                // GAME. Every reply this function returns is tried as
                // `let mut branch = position.clone(); if branch.apply(..).is_err()
                // { continue; }` — so a city with no walls left, a spent
                // strike, no Encampment, or a victim three tiles away cost a
                // full game copy to establish something the city already
                // knew. Both strikes were pushed unconditionally for every
                // enemy city at every forcing node. These are the engine's own
                // preconditions from `do_city_strike` and
                // `do_encampment_strike`, asked before the copy rather than
                // after it; the branch was discarded either way, so no search
                // value moves. Range only — line of sight and the
                // occupied-tile rules stay where they are, because those the
                // clone must still answer.
                if position.city_can_strike(city) && position.wdist(city.pos, victim_pos) <= 2 {
                    replies.push(Action::CityStrike {
                        city: city.id,
                        target: victim_pos,
                    });
                }
                for district in position.defending_districts(city) {
                    if position.defending_district_can_strike(city, &district)
                        && position.wdist(district.pos, victim_pos) <= 2
                    {
                        replies.push(
                            position
                                .defending_district_strike_action(city.id, &district, victim_pos),
                        );
                    }
                }
            }
        }
        replies
    }

    /// One-ply-then-extend search depth used to price a forcing reply: the
    /// direct reply ply, plus one further extension ply chosen by
    /// [`Self::forcing_reply_line`]'s own recursion. Named so the value only
    /// has to be justified once, at its declaration, instead of read back
    /// out of a bare `2` at the call site in
    /// [`Self::forcing_reply_penalty_from_position`].
    const FORCING_REPLY_DEPTH: usize = 2;

    /// Chess-style move-ordering width: after every forcing reply at a node
    /// is generated and scored, only this many of the strongest
    /// captures/checks are extended into another ply. Named so the value is
    /// documented once instead of read back out of a bare `.take(4)`.
    const FORCING_REPLY_WIDTH: usize = 4;

    /// Debug text for one forcing-reply candidate, used only as a sort
    /// tie-break key when two candidates already share the same primary
    /// score (see the `sort_by` in [`Self::forcing_reply_line`]). Takes the
    /// raw `Action`(s) rather than a precomputed `String` so the
    /// (allocating) `Debug` formatting happens lazily, inside the
    /// comparator's `then_with`, and only for the rare pair that actually
    /// ties — every other candidate never pays for a label at all.
    pub(super) fn forcing_reply_label(reply: &Action, followup: &Option<Action>) -> String {
        match followup {
            Some(followup) => format!("{reply:?} -> {followup:?}"),
            None => format!("{reply:?}"),
        }
    }

    pub(super) fn forcing_reply_line(
        work_pool: Option<&Arc<WorkPool>>,
        position: &Game,
        enemy: usize,
        victim: u32,
        depth: usize,
    ) -> f64 {
        if depth == 0 || !position.units.contains_key(&victim) {
            return 0.0;
        }
        let victim_hp = position.units[&victim].hp;
        let victim_pos = position.units[&victim].pos;
        let replies = Self::forcing_attacks_to(position, enemy, victim_pos, None);

        let mut reply_branches = Vec::new();
        let mut direct_attackers = BTreeSet::new();
        for reply in replies {
            let reply_unit = match &reply {
                Action::Attack { unit, .. }
                | Action::Ranged { unit, .. }
                | Action::AirStrike { unit, .. } => Some(*unit),
                _ => None,
            };
            direct_attackers.extend(reply_unit);
            let reply_hp =
                reply_unit.and_then(|unit| position.units.get(&unit).map(|candidate| candidate.hp));
            let mut branch = position.clone();
            if branch.apply(enemy, &reply).is_err() {
                continue;
            }
            // `reply` itself (not a formatted String) is stored so its Debug
            // text is only built lazily, in the sort tie-break below.
            reply_branches.push((reply, None, branch, reply_unit, reply_hp));
        }

        // A Civ unit can normally move and attack in the same turn. Search
        // only one-step forcing approaches whose resulting position already
        // has a legal attack on the victim. This is the tactical analogue of
        // a check extension: it closes the horizon gap around an exposed
        // capture without admitting every quiet movement into quiescence.
        let mobile_attackers: Vec<u32> = position
            .units
            .values()
            .filter(|unit| unit.owner == enemy && !direct_attackers.contains(&unit.id))
            .filter(|unit| {
                let spec = &position.rules.units[unit.kind];
                spec.class == "military"
                    && spec.domain.as_deref() != Some("air")
                    && position.wdist(unit.pos, victim_pos)
                        <= position.unit_attack_range(unit.id) + 2
            })
            .map(|unit| unit.id)
            .collect();
        for attacker in mobile_attackers {
            let reply_hp = position.units[&attacker].hp;
            for to in position
                .nbrs(position.units[&attacker].pos)
                .into_iter()
                .filter(|to| position.can_move(attacker, *to))
            {
                let movement = Action::Move { unit: attacker, to };
                let mut moved = position.clone();
                if moved.apply(enemy, &movement).is_err() {
                    continue;
                }
                let followups = Self::forcing_attacks_to(&moved, enemy, victim_pos, Some(attacker));
                for followup in followups {
                    let mut branch = moved.speculative_clone();
                    if branch.apply(enemy, &followup).is_err() {
                        continue;
                    }
                    reply_branches.push((
                        movement.clone(),
                        Some(followup),
                        branch,
                        Some(attacker),
                        Some(reply_hp),
                    ));
                }
            }
        }

        let mut ordered = Vec::new();
        for (reply, followup, branch, reply_unit, reply_hp) in reply_branches {
            let loss = branch
                .units
                .get(&victim)
                .map(|unit| (victim_hp - unit.hp).max(0) as f64)
                .unwrap_or(victim_hp as f64 + 35.0);
            let counter_loss = match (reply_unit, reply_hp) {
                (Some(unit), Some(hp)) => branch
                    .units
                    .get(&unit)
                    .map(|unit| (hp - unit.hp).max(0) as f64)
                    .unwrap_or(hp as f64 + 20.0),
                _ => 0.0,
            };
            ordered.push((
                (loss - 0.35 * counter_loss).max(0.0),
                reply,
                followup,
                branch,
            ));
        }

        // Chess-style move ordering keeps the extension bounded: examine all
        // forcing replies at the frontier, but only extend the
        // FORCING_REPLY_WIDTH strongest captures/checks into another
        // focus-fire action. The tie-break Debug text is built lazily,
        // inside `then_with`, so it is only paid for the rare pair of
        // candidates that actually share the same primary score.
        ordered.sort_by(|left, right| {
            right.0.total_cmp(&left.0).then_with(|| {
                Self::forcing_reply_label(&left.1, &left.2)
                    .cmp(&Self::forcing_reply_label(&right.1, &right.2))
            })
        });
        let continuations = ordered
            .into_iter()
            .take(Self::FORCING_REPLY_WIDTH)
            .collect::<Vec<_>>();
        let values = match work_pool {
            Some(pool) if continuations.len() > 1 => {
                let nested_pool = Arc::clone(pool);
                pool.map_owned(continuations, move |(immediate, _, _, branch)| {
                    immediate
                        + Self::forcing_reply_line(
                            Some(&nested_pool),
                            &branch,
                            enemy,
                            victim,
                            depth - 1,
                        )
                })
            }
            _ => continuations
                .into_iter()
                .map(|(immediate, _, _, branch)| {
                    immediate
                        + Self::forcing_reply_line(work_pool, &branch, enemy, victim, depth - 1)
                })
                .collect(),
        };
        values.into_iter().fold(0.0_f64, f64::max)
    }

    pub(super) fn note_illegal_attack(why: &str) {
        if let Ok(mut tally) = Self::illegal_attack_tally().lock() {
            *tally.entry(why.to_string()).or_insert(0) += 1;
        }
    }

    /// Price the enemy's forcing reply on a board an exact attack has already
    /// produced, instead of on a second clone of the same world.
    ///
    /// ⚠ EXACT BY CONSTRUCTION, AND THE ARGUMENT IS SHORT: `Game::apply` is
    /// the ONLY mutation `tactical_attack_result_in` makes — everything else
    /// it touches is a read — so the board it hands back is bit-for-bit the
    /// board `forcing_reply_penalty_owned` used to build by cloning again and
    /// applying the same action. The three arms below reproduce that function's
    /// behaviour case for case, including the refusal reason it reports and the
    /// 135.0 it charges an attacker that did not survive its own attack.
    pub(super) fn forcing_reply_penalty_applied(
        work_pool: Option<Arc<WorkPool>>,
        after: &mut Game,
        applied: AppliedAttack,
        pid: usize,
        uid: u32,
        action: &Action,
    ) -> f64 {
        match applied {
            AppliedAttack::Refused(why) => {
                Self::note_illegal_attack(&why);
                return 1_000.0;
            }
            AppliedAttack::NotScored => {
                // Nothing was applied, because there was no attack to score.
                // This is the one arm that still pays an apply, and it is the
                // arm that used to pay a clone as well.
                if let Err(why) = after.apply(pid, action) {
                    Self::note_illegal_attack(&why);
                    return 1_000.0;
                }
            }
            AppliedAttack::Applied => {}
        }
        if !after.units.contains_key(&uid) {
            return 135.0;
        }
        Self::forcing_reply_penalty_from_position(work_pool.as_ref(), after, pid, &[uid])
    }

    /// Price the strongest forcing reply from an already-resolved friendly
    /// position.  Most callers have one exposed attacker; a coordinated
    /// volley passes both surviving bodies so the opponent is allowed to pick
    /// whichever member of the pair is actually vulnerable.
    pub(super) fn forcing_reply_penalty_from_position(
        work_pool: Option<&Arc<WorkPool>>,
        after: &Game,
        pid: usize,
        victims: &[u32],
    ) -> f64 {
        let victims: Vec<u32> = victims
            .iter()
            .copied()
            .filter(|uid| after.units.contains_key(uid))
            .collect();
        if victims.is_empty() {
            return 0.0;
        }
        let enemies: Vec<usize> = after
            .players
            .iter()
            .filter(|player| player.id != pid && player.alive && after.is_at_war(pid, player.id))
            .map(|player| player.id)
            .collect();
        let mut worst_reply = 0.0_f64;
        // With one front there is no clone to save, so retain the original
        // path exactly. The broad geometry scan only earns its keep when it
        // can eliminate one or more other warring seats.
        let has_multiple_enemies = enemies.len() > 1;

        for enemy in enemies {
            if has_multiple_enemies
                && !Self::enemy_can_force_a_reply_against_any(after, enemy, &victims)
            {
                continue;
            }
            let mut reply_position = after.speculative_clone();
            reply_position.current = enemy;
            for unit in reply_position
                .units
                .values_mut()
                .filter(|unit| unit.owner == enemy)
            {
                // Only attacks are searched, so a generous movement budget
                // merely restores next-turn attack availability; it cannot
                // manufacture a move-and-attack line in this one-ply search.
                unit.moves_left = 100.0;
                unit.attacks_left = 1;
                unit.acted = false;
                unit.zoc_stopped = false;
            }
            for city in reply_position
                .cities
                .values_mut()
                .filter(|city| city.owner == enemy)
            {
                city.struck = false;
                city.encampment_struck = false;
                city.encampment_extra_strikes_used = 0;
                for district in &mut city.defending_districts {
                    district.struck = false;
                    district.extra_strikes_used = 0;
                }
            }
            for victim in &victims {
                worst_reply = worst_reply.max(Self::forcing_reply_line(
                    work_pool,
                    &reply_position,
                    enemy,
                    *victim,
                    Self::FORCING_REPLY_DEPTH,
                ));
            }
        }
        worst_reply
    }

    /// Whether an enemy can *possibly* make one of the forcing replies this
    /// search admits on its next turn.  A negative answer is enough to avoid
    /// cloning the whole board for that enemy; a positive answer deliberately
    /// remains only a broad prefilter and the cloned `Game::apply` below is
    /// still the authority on terrain, line of sight, occupancy, and costs.
    ///
    /// The reply position resets the enemy's moves, attacks, and city strikes
    /// before searching it.  This predicate must therefore not use their
    /// current availability.  It mirrors the geometry of `forcing_attacks_to`:
    /// direct air strikes, direct or one-step land attacks, and city or
    /// encampment strikes.  City checks are intentionally looser than the
    /// engine (range only), which preserves every uncertain branch rather than
    /// proving a legal counterattack away from the search.
    pub(super) fn enemy_can_force_a_reply_against_any(
        position: &Game,
        enemy: usize,
        victims: &[u32],
    ) -> bool {
        let unit_can_reach_a_victim = position
            .units
            .values()
            .filter(|unit| unit.owner == enemy)
            .any(|unit| {
                let spec = &position.rules.units[unit.kind];
                let reach = position.unit_attack_range(unit.id);
                if spec.domain.as_deref() == Some("air") {
                    return victims.iter().any(|victim| {
                        position
                            .units
                            .get(victim)
                            .is_some_and(|victim| position.wdist(unit.pos, victim.pos) <= reach)
                    });
                }
                spec.class == "military"
                    && victims.iter().any(|victim| {
                        position
                            .units
                            .get(victim)
                            .is_some_and(|victim| position.wdist(unit.pos, victim.pos) <= reach + 2)
                    })
                    && !position.is_embarked(unit)
            });
        if unit_can_reach_a_victim {
            return true;
        }

        position
            .cities
            .values()
            .filter(|city| city.owner == enemy)
            .any(|city| {
                victims.iter().any(|victim| {
                    let Some(victim) = position.units.get(victim) else {
                        return false;
                    };
                    position.wdist(city.pos, victim.pos) <= 2
                        || position
                            .defending_districts(city)
                            .any(|district| position.wdist(district.pos, victim.pos) <= 2)
                })
            })
    }

    /// Price a forcing reply against an untouched board, cloning and applying
    /// the action here.
    ///
    /// The tactical picker no longer takes this route — it hands over the board
    /// its own attack evaluation already applied — so this is the entry point
    /// for a caller holding nothing but `&Game`. `AppliedAttack::NotScored` is
    /// exactly that statement: nothing has been applied to this board yet.
    ///
    /// Production uses the applied form, while the focused regression tests
    /// retain this `&Game` door to assert the same reply cost directly. Keep
    /// that test contract out of release builds.
    #[cfg(test)]
    pub(super) fn forcing_reply_penalty(
        &self,
        g: &Game,
        pid: usize,
        uid: u32,
        action: &Action,
    ) -> f64 {
        let mut after = g.speculative_clone();
        Self::forcing_reply_penalty_applied(
            self.work_pool.clone(),
            &mut after,
            AppliedAttack::NotScored,
            pid,
            uid,
            action,
        )
    }
}
