//! `culture-builds-through-a-quiet-war`: a Culture seat at war with a major
//! that has not come near keeps its culture claims (first Theater Squares,
//! Amphitheaters and first Art Museum, Monuments, the Faith sources opened at
//! two Theaters) running, as it would at peace.
//!
//! **Why (live Deity Online Kristina, 2026-11-10, six Sweden culture seats).**
//! Four of six were at war with a major before t95, and
//! `threatened-war-holds-the-claims` held every economic claim, the culture
//! claims among them, for 31, 78 and 54 turns in three of them
//! (civvis-20261110T141854Z, 142605Z, 144009Z). The hold reads the plan's
//! Recovery strategy ("at war and losing ground at home"), which follows the
//! power ratio, not contact: in 142605Z Nubia declared at t43, no Nubian unit
//! was ever in our view from t45 to t80 (the state's hostiles held only
//! barbarians), and the claims stood aside until t128. Of the held turns,
//! 25 of 31, 65 of 78 and 41 of 54 had no city of ours damaged. The Monument
//! claim also returns outright at any major war, and the Theater claim treats
//! every military head as untouchable. Culture a turn read 8, 12 and 18 at
//! t50, t70 and t100 in 142605Z, whose first Theater Square stood at t72.
//!
//! **What the gene does.** A war is quiet while no city of ours is the
//! plan's threatened city, none was attacked within
//! [`QUIET_WAR_STANDARD_TURNS`] standard turns, and no military unit of a
//! major at war with us stands within [`QUIET_WAR_RADIUS`] tiles of one. In a
//! quiet war, on a Culture seat, the Theater and Monument claims run past
//! `threatened-war-holds-the-claims`, and the Theater, Amphitheater, Art
//! Museum, Monument and Faith-source claims read the war as peace when they
//! ask whether a head may wait (a military unit keeps its progress and comes
//! back after the culture build). The war's own claims, every other hold,
//! the end-of-turn defence redirects and the threatened city's defender are
//! unchanged. Off, at peace, or for another victory target, nothing changes.

use super::*;

/// Standard turns since a city of ours was last attacked before a war reads
/// as quiet: ten Online turns.
pub(crate) const QUIET_WAR_STANDARD_TURNS: u32 = 15;

/// Tiles from a city of ours within which an enemy major's soldier keeps a
/// war loud: the faith-source claim's own safety radius.
pub(crate) const QUIET_WAR_RADIUS: i32 = 6;

impl AdvancedAi {
    /// Whether this is a quiet war for the culture claims: the gene is on,
    /// the seat aims at Culture, a living major is at war with us and none
    /// of its soldiers, nor any recent attack, reaches a city of ours.
    pub(super) fn culture_quiet_war(&self, g: &Game, pid: usize, plan: &StrategicPlan) -> bool {
        if !self.culture_builds_through_a_quiet_war
            || self.active_victory_target(g) != Some(VictoryTarget::Culture)
            || plan.threatened_city.is_some()
        {
            return false;
        }
        let enemies: BTreeSet<usize> = g
            .players
            .iter()
            .filter(|other| {
                other.id != pid
                    && other.alive
                    && !other.is_minor
                    && !other.is_barbarian
                    && g.is_at_war(pid, other.id)
            })
            .map(|other| other.id)
            .collect();
        if enemies.is_empty() {
            return false;
        }
        let memory = g.standard_duration(QUIET_WAR_STANDARD_TURNS);
        let homes: Vec<Pos> = g
            .player_city_ids(pid)
            .into_iter()
            .map(|cid| &g.cities[&cid])
            .filter(|city| {
                if city.last_attacked > 0 && g.turn.saturating_sub(city.last_attacked) <= memory {
                    return false;
                }
                true
            })
            .map(|city| city.pos)
            .collect();
        if homes.len() != g.player_city_ids(pid).len() {
            return false;
        }
        !g.units.values().any(|unit| {
            enemies.contains(&unit.owner)
                && g.rules
                    .units
                    .get(&unit.kind)
                    .is_some_and(|spec| spec.class == "military")
                && homes
                    .iter()
                    .any(|home| g.wdist(*home, unit.pos) <= QUIET_WAR_RADIUS)
        })
    }

    /// Whether the culture claims read a major war as one: true at a major
    /// war unless this gene finds it quiet.
    pub(super) fn culture_reads_a_major_war(
        &self,
        g: &Game,
        pid: usize,
        plan: &StrategicPlan,
    ) -> bool {
        g.players.iter().any(|other| {
            other.id != pid
                && other.alive
                && !other.is_barbarian
                && !other.is_minor
                && g.is_at_war(pid, other.id)
        }) && !self.culture_quiet_war(g, pid, plan)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn game() -> Game {
        Game::new_full(2, 30, 20, 79_208, 250, 0, false)
    }

    fn seat(on: bool) -> AdvancedAi {
        let mut ai = AdvancedAi::targeting(VictoryTarget::Culture);
        if on {
            ai.enable_culture_builds_through_a_quiet_war();
        }
        ai
    }

    #[test]
    fn off_at_peace_or_another_target_is_never_quiet() {
        let mut g = game();
        let plan = seat(true).assess(&g, 0);
        assert!(!seat(true).culture_quiet_war(&g, 0, &plan), "at peace");
        g.at_war.insert((0, 1));
        assert!(!seat(false).culture_quiet_war(&g, 0, &plan), "off");
        assert!(seat(false).culture_reads_a_major_war(&g, 0, &plan));
        let mut science = AdvancedAi::targeting(VictoryTarget::Science);
        science.enable_culture_builds_through_a_quiet_war();
        assert!(!science.culture_quiet_war(&g, 0, &plan), "another target");
    }

    /// A war whose soldiers stay away is quiet; a soldier near a city, or a
    /// recent attack on one, is not.
    #[test]
    fn a_war_is_quiet_until_it_comes_near() {
        let mut g = Game::new_full(2, 32, 20, 936_221, 200, 0, false);
        let settler = g
            .player_unit_ids(0)
            .into_iter()
            .find(|uid| g.units[uid].kind == "settler")
            .expect("fixture: a settler");
        g.apply(0, &Action::FoundCity { unit: settler })
            .expect("fixture: the capital founds");
        for pid in [0, 1] {
            for uid in g.player_unit_ids(pid) {
                g.remove_unit(uid);
            }
        }
        let ours = g.player_city_ids(0)[0];
        let home = g.cities[&ours].pos;
        g.at_war.insert((0, 1));
        let ai = seat(true);
        let mut plan = ai.assess(&g, 0);
        plan.threatened_city = None;
        assert!(ai.culture_quiet_war(&g, 0, &plan), "no soldier near: quiet");
        assert!(!ai.culture_reads_a_major_war(&g, 0, &plan));

        let near = g
            .map
            .tiles
            .iter()
            .filter(|(pos, tile)| g.wdist(**pos, home) == 3 && !g.rules.is_water(tile))
            .map(|(pos, _)| *pos)
            .min()
            .expect("fixture: land three tiles out");
        let mut loud = g.clone();
        loud.spawn_unit("warrior", 1, near);
        assert!(!ai.culture_quiet_war(&loud, 0, &plan), "a soldier near");
        assert!(ai.culture_reads_a_major_war(&loud, 0, &plan));

        let mut struck = g.clone();
        struck.turn = 40;
        struck.cities.get_mut(&ours).unwrap().last_attacked = 38;
        assert!(!ai.culture_quiet_war(&struck, 0, &plan), "a recent attack");
    }
}
