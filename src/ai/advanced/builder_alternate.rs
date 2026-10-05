//! A refused preferred route need not consume the Builder's work turn.
//! This experiment completes another ordinary worthwhile improvement only
//! when it pays Production immediately without losing city Food or Science.
//! Existing emergency, project, repair, support and route decisions run first.

use super::*;

const ALTERNATE_PRICE_ATTEMPTS: usize = 6;

impl AdvancedAi {
    pub(super) fn builder_productive_alternate_step(
        &mut self,
        g: &mut Game,
        pid: usize,
        uid: u32,
        strategy: GrandStrategy,
        reserved: &HashSet<Pos>,
    ) -> bool {
        if !self.builder_productive_alternate
            || self.active_victory_target(g).is_none()
            || self.builder_support.contains_key(&uid)
            || !g.units.get(&uid).is_some_and(|unit| {
                unit.owner == pid
                    && unit.kind == "builder"
                    && unit.moves_left > 0.0
                    && unit.charges > 0
            })
        {
            return false;
        }
        let current = g.units[&uid].pos;
        // Use the ordinary capture model even if a screened route gene is off.
        // No guard is recruited or borrowed by this fallback.
        let reach = self.barbarian_reach(g, pid, current, civilian_safety::REACH_SCAN_RADIUS);
        let visible = self.battlefront_visibility(g, pid);
        let threats = self.visible_barbarian_capture_threats(g, pid, &visible);
        let mut candidates = Vec::new();
        for pos in g.reachable(uid).into_iter().chain([current]) {
            if reserved.contains(&pos) {
                continue;
            }
            let Some(city) = g
                .map
                .get(pos)
                .and_then(|tile| tile.owner_city)
                .filter(|city| g.cities.get(city).is_some_and(|city| city.owner == pid))
            else {
                continue;
            };
            let Some(path) = g.path_to(uid, pos) else {
                continue;
            };
            if !path.iter().chain(std::iter::once(&pos)).all(|step| {
                self.builder_job_out_of_reach(g, pid, uid, *step, &reach)
                    && self.builder_barbarian_capture_risk_with_threats(
                        g, pid, uid, *step, &visible, &threats,
                    ) <= BUILDER_BARBARIAN_CAPTURE_RISK_LIMIT
            }) {
                continue;
            }
            let shortfall = self.city_production_foundation_shortfall(g, pid, city);
            // Preserve resource/boost/quest premiums and normal replacement
            // eligibility; this is an alternate route, not a new job price.
            for improvement in self.worthwhile_improvements(g, pid, pos, strategy) {
                let score = self.production_foundation_improvement_value(
                    g,
                    pid,
                    pos,
                    &improvement,
                    strategy,
                    shortfall,
                ) - f64::from(g.wdist(current, pos)) * 0.7;
                candidates.push((score, pos, city, improvement));
            }
        }
        candidates.sort_by(|left, right| {
            right
                .0
                .total_cmp(&left.0)
                .then(left.1.cmp(&right.1))
                .then(left.3.cmp(&right.3))
        });
        candidates.dedup_by(|left, right| left.1 == right.1 && left.3 == right.3);
        for (_, pos, city, improvement) in candidates.into_iter().take(ALTERNATE_PRICE_ATTEMPTS) {
            let mut trial = g.speculative_clone();
            if pos != current
                && (trial
                    .apply(pid, &Action::MoveTo { unit: uid, to: pos })
                    .is_err()
                    || trial
                        .units
                        .get(&uid)
                        .is_none_or(|unit| unit.pos != pos || unit.moves_left <= 0.0))
            {
                continue;
            }
            // Attribute only the operation. Walking can itself change yields
            // through a village reward; it must not inflate the improvement.
            let before = trial.city_yields(city);
            let action = Action::Improve {
                unit: uid,
                improvement,
            };
            if trial.apply(pid, &action).is_err() {
                continue;
            }
            let after = trial.city_yields(city);
            if after.production <= before.production + 1e-9
                || after.food < before.food - 1e-9
                || after.science < before.science - 1e-9
            {
                continue;
            }
            let walked = pos != current;
            if walked && !self.base.path_walk_to(g, pid, uid, pos) {
                continue;
            }
            if g.units
                .get(&uid)
                .is_none_or(|unit| unit.pos != pos || unit.moves_left <= 0.0)
            {
                return walked;
            }
            if g.apply(pid, &action).is_ok() {
                self.builder_targets.remove(&uid);
                self.note_first_luxury_opened(g, pid, pos, &improvement);
                think!(self.journal(), Expansion, Decision, "Builder completes productive alternate work";
                    "Builder {uid} improves {improvement} at {pos:?}; after-walk city Production +{:.2}, Food and Science retained",
                    after.production - before.production; pos);
                return true;
            }
            if walked {
                self.builder_targets.insert(uid, pos);
                return true;
            }
        }
        false
    }
}

#[cfg(test)]
mod tests;
