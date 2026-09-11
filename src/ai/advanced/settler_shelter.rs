//! A worthwhile city is another escape from civilian capture.
use super::{civilian_safety::REACH_SCAN_RADIUS, Action, AdvancedAi, Game};
use crate::think;

impl AdvancedAi {
    pub(super) fn settler_founds_for_shelter(
        &mut self,
        g: &mut Game,
        pid: usize,
        uid: u32,
    ) -> bool {
        // Repair the live expansion walker without changing frozen native
        // policies or the capital's separate opening-site search.
        if !self.live_settler_capture_lessons
            || !self.settlement_safety
            || g.player_city_ids(pid).is_empty()
            || !g.units.get(&uid).is_some_and(|unit| unit.moves_left > 0.0)
            || !g.can_found_city(uid)
        {
            return false;
        }
        let here = g.units[&uid].pos;
        let reach = self.barbarian_reach(g, pid, here, REACH_SCAN_RADIUS);
        if !reach.covers(g, here)
            || self.settler_site_is_dead(uid, here)
            || self.settle_site_loyalty_verdict(g, pid, here).is_some()
            || self
                .live_stalled_settlement_is_unsupported_frontline(g, pid, here)
                .is_some()
        {
            return false;
        }
        // Price the actual city site, not the civilian's exposure on it.
        // Reusing the expansion floor keeps wasteland out of this fallback.
        let worth = self.settlement_static_value_uncached(g, pid, here);
        if worth < Self::settler_site_gate_floor(g.player_city_ids(pid).len()) {
            return false;
        }
        // Ask the engine for the founded city's HP and strength on a private
        // board. This neither consumes live RNG nor emits a speculative order.
        // The bounded, rare emergency path avoids duplicating city formulas.
        let mut projected = g.clone();
        if projected
            .apply(pid, &Action::FoundCity { unit: uid })
            .is_err()
        {
            return false;
        }
        let Some(cid) = projected.city_at(here) else {
            return false;
        };
        let city = &projected.cities[&cid];
        let defense = projected.city_strength(cid);
        let visible = self.battlefront_visibility(g, pid);
        let mut pressure = 0.0;
        let mut seen_threat = false;
        for unit in g.units.values() {
            let spec = &g.rules.units[unit.kind];
            if unit.owner == pid
                || !g.is_at_war(pid, unit.owner)
                || !g.sees(&visible, unit.pos)
                || !g.unit_visible_to(unit.id, pid)
                || spec.class != "military"
                || g.wdist(here, unit.pos) > spec.moves.ceil() as i32 + spec.range.max(1)
            {
                continue;
            }
            seen_threat = true;
            let attack = g
                .unit_strength(unit, false)
                .max(g.unit_ranged_attack_strength(unit));
            // An upper-roll estimate for every nearby attacker, with a quarter
            // of the city's HP left as margin. Do not call a fresh city safe
            // merely because a civilian could no longer be captured outright.
            pressure += (1.2 * crate::game::expected_damage(attack, defense)).min(100.0);
        }
        // A remembered raider alone has no current strength/location proof.
        if !seen_threat || pressure >= f64::from(city.hp) * 0.75 {
            return false;
        }
        if g.apply(pid, &Action::FoundCity { unit: uid }).is_err() {
            return false;
        }
        self.settler_targets.remove(&uid);
        self.settler_relaxed_targets.remove(&uid);
        self.settler_stalls.remove(&uid);
        self.settler_blocked_turns.remove(&uid);
        self.settler_closest.remove(&uid);
        self.settler_retreats.remove(&uid);
        think!(self.journal(), Expansion, Decision,
               "Settler founds a city for shelter at {here:?}";
               "the site is worth {worth:.1}; estimated nearby attack damage is \
                {pressure:.1} against {} city HP, so founding beats exposing the settler \
                for another turn", city.hp; here);
        true
    }
}

#[cfg(test)]
mod tests;
