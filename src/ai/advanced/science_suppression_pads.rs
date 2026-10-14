//! `science-suppression-hits-the-pads`: a space race is suppressed at a pad.
//!
//! `victory_suppression_city` names the first campaign objective (and the air
//! surge's target) from the lane the counter reads the rival in: a Spaceport
//! city for Science, a Theatre Square city for Culture. A rival racing on two
//! lanes reads in whichever is higher, and once the Exoplanet Expedition is in
//! flight nothing in the host stops it short of conquering that player (the
//! game's own advisor text: "the only way to prevent a player from achieving
//! a Science Victory is to win another kind of Victory first, or conquer that
//! player"). The siege has to land on a pad before the launch.
//!
//! Live Emperor G408 (civvis-20261008T143512Z) lost to Nubia's Science
//! Victory at turn 241. Nubia landed the Moon by 190, the Mars base by about
//! 220 and launched the Exoplanet about 233, while the counter read it on
//! Culture (89 against the Science clock's 84 after Mars). At 238 the
//! campaign's first objective was Faras, a Theatre Square city, and the one
//! Nubian pad our spy had already pillaged at 190 and that stayed pillaged to
//! the end. The standing pads at Meroë and by (32, 5) were never named.
//!
//! Against a decisive space racer — the every-pad trigger: two space projects
//! landed or a science race reading 80 — this names the rival's city with a
//! standing (unpillaged) Spaceport that passes the same reach gates the lane
//! choice does (the declaration range while at peace, the capture deferral),
//! nearest to one of our cities first, then the most productive, whatever
//! lane the rival reads in. With no such city it leaves the lane's choice.
//!
//! The counter has to name the racer first. Live G411
//! (civvis-20261008T151915Z) held "Countering Byzantium | culture 67-79%"
//! from turn 172 while Mongolia, at war with us, had its Mars base landed by
//! turn 180, launched the Exoplanet by 200 and won on Science at 212. A
//! one-turn replay of turn 186 under the Domination target names Byzantium
//! too: `domination_military_counter` sorts the clocks by reading, and
//! Byzantium's nearest-finish culture clock read 85 against Mongolia's 84
//! (the Science clock's median nineteen turns after Mars). But a Science race
//! closes at the launch, not the finish: past it no war counts. So a rival
//! with the Mars base landed and the Exoplanet not yet launched is the
//! Domination army's counter before any other clock, the most advanced first,
//! when a Conquest counter against it is actionable.

use super::science_denial_every_pad::{EVERY_PAD_MIN_PRESSURE, EVERY_PAD_MIN_STAGES};
use super::*;

impl AdvancedAi {
    /// See the module: the Mars-landed, not-yet-launched racer a Domination
    /// army counters before any other clock, with the most launches landed
    /// (then the lower seat). `None` with the gene off, outside a Domination
    /// plan, or when no Conquest counter against such a rival is actionable.
    pub(super) fn mars_racer_counter(
        &self,
        g: &Game,
        pid: usize,
    ) -> Option<(usize, GrandStrategy)> {
        if !self.science_suppression_hits_the_pads
            || !self.deny_leaders
            || !g.victory_conditions.science
            || self.active_victory_target(g) != Some(VictoryTarget::Domination)
        {
            return None;
        }
        g.players
            .iter()
            .filter(|rival| {
                rival.id != pid
                    && rival.alive
                    && !rival.is_minor
                    && !rival.is_barbarian
                    && !g.same_team(pid, rival.id)
                    && g.has_met(pid, rival.id)
                    && rival.science_projects.contains("launch_mars_colony")
                    && !rival.science_projects.contains("exoplanet_expedition")
            })
            .filter(|rival| {
                self.conquest_denial_actionable(g, pid, rival.id, GrandStrategy::Conquest)
            })
            .max_by_key(|rival| {
                (
                    Self::science_denial_stages(g, rival.id),
                    std::cmp::Reverse(rival.id),
                )
            })
            .map(|rival| (rival.id, GrandStrategy::Conquest))
    }

    /// See the module. `None` with the gene off, against a rival that is no
    /// decisive space racer, or when it stands no reachable pad.
    pub(super) fn suppression_pad_city(&self, g: &Game, pid: usize, rival: usize) -> Option<u32> {
        if !self.science_suppression_hits_the_pads || !g.victory_conditions.science {
            return None;
        }
        let decisive = Self::science_denial_stages(g, rival) >= EVERY_PAD_MIN_STAGES || {
            let pressure = self.rival_victory_pressure(g, rival);
            pressure.strategy == GrandStrategy::Science
                && pressure.progress >= EVERY_PAD_MIN_PRESSURE
        };
        if !decisive {
            return None;
        }
        let ours: Vec<Pos> = g
            .player_city_ids(pid)
            .iter()
            .map(|cid| g.cities[cid].pos)
            .collect();
        let at_war = g.is_at_war(pid, rival);
        g.cities
            .values()
            .filter(|city| city.owner == rival)
            .filter(|city| {
                city.districts.iter().any(|(district, pad)| {
                    g.district_family(*district) == "spaceport"
                        && g.map.get(*pad).is_some_and(|tile| !tile.pillaged)
                })
            })
            .filter(|city| at_war || Self::city_within_declaration_range(g, pid, city.pos))
            .filter(|city| !Self::should_defer_city_capture(g, pid, city.id))
            .map(|city| {
                let reach = ours
                    .iter()
                    .map(|home| g.wdist(*home, city.pos))
                    .min()
                    .unwrap_or(i32::MAX);
                (reach, g.city_yields(city.id).production, city.id)
            })
            .min_by(|left, right| {
                left.0
                    .cmp(&right.0)
                    .then(right.1.total_cmp(&left.1))
                    .then(left.2.cmp(&right.2))
            })
            .map(|(_, _, city)| city)
    }
}

#[cfg(test)]
mod tests;
