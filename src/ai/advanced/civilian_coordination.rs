//! Shared intentions for a threatened live Builder and its military guard.
//! Settlers already bind escorts. Builders need the same reservation boundary:
//! choose a common end tile before battle planning, then validate both walks
//! together. Rebuild on every observed frame instead of trusting stale unit IDs
//! or a simulated kill to have made the next host turn safe.

use super::*;

#[derive(Clone, Copy, Debug)]
pub(super) struct BuilderSupport {
    pub(super) guard: u32,
    destination: Pos,
}

impl AdvancedAi {
    pub(super) fn builder_guard_reserved(&self, guard: u32) -> bool {
        self.builder_support
            .values()
            .any(|support| support.guard == guard)
    }

    pub(super) fn guard_is_reserved_for_civilian(&self, guard: u32) -> bool {
        self.guard_is_bound_to_any_settler(guard) || self.builder_guard_reserved(guard)
    }

    pub(super) fn plan_builder_support(&mut self, g: &Game, pid: usize) {
        self.builder_support.clear();
        if !self.live_settler_capture_lessons {
            return;
        }
        let mut danger = None;
        for builder in g.player_unit_ids(pid) {
            let unit = &g.units[&builder];
            let current = unit.pos;
            if unit.kind != "builder" || g.city_at(current).is_some() {
                continue;
            }
            let reach = self.barbarian_reach(g, pid, current, civilian_safety::REACH_SCAN_RADIUS);
            if !reach.covers(g, current) {
                continue;
            }
            let mut destinations = g.reachable(builder);
            destinations.push(current);
            destinations.sort_unstable();
            destinations.dedup();
            // An independent escape leaves the soldier available for battle.
            if destinations.iter().any(|pos| {
                *pos != current
                    && (!reach.covers(g, *pos)
                        || g.city_at(*pos).is_some_and(|id| g.cities[&id].owner == pid))
                    && g.path_to(builder, *pos).is_some()
            }) {
                continue;
            }
            let home = g
                .player_city_ids(pid)
                .into_iter()
                .map(|id| g.cities[&id].pos)
                .min_by_key(|pos| (g.wdist(current, *pos), *pos));
            let danger =
                danger.get_or_insert_with(|| battle_planner::DangerField::with_reach(g, pid, true));
            let mut candidates = Vec::new();
            for guard in g.unit_ids_at(current).iter().copied() {
                let soldier = &g.units[&guard];
                if soldier.owner != pid
                    || !Self::guard_matches_escort_layer(g, soldier, false)
                    || soldier.hp < STACKED_GUARD_MIN_HP
                    || soldier.linked_to.is_some()
                    || self.guard_is_reserved_for_civilian(guard)
                {
                    continue;
                }
                let reachable: BTreeSet<_> =
                    g.reachable(guard).into_iter().chain([current]).collect();
                for &destination in &destinations {
                    if !reachable.contains(&destination)
                        || self
                            .builder_support
                            .values()
                            .any(|support| support.destination == destination)
                        || g.map
                            .get(destination)
                            .is_none_or(|tile| g.rules.is_water(tile))
                    {
                        continue;
                    }
                    let expected = danger.danger(destination, guard);
                    if expected + 5.0 >= f64::from(soldier.hp) {
                        continue;
                    }
                    let support = BuilderSupport { guard, destination };
                    if Self::builder_support_actions(g, pid, builder, support).is_none() {
                        continue;
                    }
                    candidates.push((
                        expected,
                        home.map_or(0, |home| g.wdist(destination, home)),
                        g.wdist(current, destination),
                        guard,
                        destination,
                    ));
                }
            }
            candidates.sort_by(|a, b| {
                a.0.total_cmp(&b.0)
                    .then_with(|| a.1.cmp(&b.1))
                    .then_with(|| a.2.cmp(&b.2))
                    .then_with(|| a.3.cmp(&b.3))
                    .then_with(|| a.4.cmp(&b.4))
            });
            if let Some(&(_, _, _, guard, destination)) = candidates.first() {
                self.builder_support
                    .insert(builder, BuilderSupport { guard, destination });
                think!(self.journal(), Expansion, Detail, "Builder and guard share a plan";
                       "builder {builder} and guard {guard} reserve {destination:?}; both routes fit this turn and military planning cannot spend the guard"; destination);
            }
        }
    }

    fn builder_support_actions(
        g: &Game,
        pid: usize,
        builder: u32,
        support: BuilderSupport,
    ) -> Option<Vec<Action>> {
        let mut trial = g.speculative_clone();
        let mut actions = Vec::new();
        for uid in [support.guard, builder] {
            let unit = trial.units.get(&uid)?;
            if unit.owner != pid {
                return None;
            }
            if unit.pos != support.destination {
                let action = Action::MoveTo {
                    unit: uid,
                    to: support.destination,
                };
                trial.apply(pid, &action).ok()?;
                if trial.units.get(&uid)?.pos != support.destination {
                    return None;
                }
                actions.push(action);
            }
        }
        Some(actions)
    }

    pub(super) fn builder_support_step(
        &mut self,
        g: &mut Game,
        pid: usize,
        builder: u32,
    ) -> Option<bool> {
        let support = *self.builder_support.get(&builder)?;
        let Some(actions) = Self::builder_support_actions(g, pid, builder, support) else {
            // A failed joint walk must not release the guard and recreate
            // the very abandonment this reservation prevents. If the pair
            // can still hold safely, keep both at their observed position.
            if let Some(current) = g.units.get(&builder).map(|unit| unit.pos) {
                self.builder_support.insert(
                    builder,
                    BuilderSupport {
                        destination: current,
                        ..support
                    },
                );
                if self.builder_support_protects(g, pid, builder, current) {
                    think!(self.journal(), Expansion, Detail, "Builder and guard hold together";
                           "their shared walk no longer fits; the guard stays reserved at {current:?}"; current);
                    return Some(false);
                }
            }
            self.builder_support.remove(&builder);
            return None;
        };
        if actions.is_empty() {
            return None;
        }
        // Both actions were checked on one clone of this exact board. The
        // guard walks first; the Builder's order follows to the same endpoint.
        for action in actions {
            g.apply(pid, &action)
                .expect("joint walk validated on the unchanged board");
        }
        Some(true)
    }

    pub(super) fn builder_support_protects(
        &self,
        g: &Game,
        pid: usize,
        builder: u32,
        pos: Pos,
    ) -> bool {
        self.builder_support.get(&builder).is_some_and(|support| {
            support.destination == pos
                && g.units.get(&support.guard).is_some_and(|guard| {
                    guard.owner == pid
                        && guard.pos == pos
                        && guard.hp >= STACKED_GUARD_MIN_HP
                        && battle_planner::strike_danger(g, pid, pos, guard.id) + 5.0
                            < f64::from(guard.hp)
                })
        })
    }
}

#[cfg(test)]
mod tests;
