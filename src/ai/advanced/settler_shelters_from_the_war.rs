//! `settler-shelters-from-the-war`: a Settler in the open while an enemy
//! major's soldier is near goes to the nearest city of ours, and a Settler
//! already inside one stays there.
//!
//! **The finding (live Deity Korea, 2026-11-04/05).** 35 Settlers were
//! captured in the Deity games since 11-04 21Z, and 23 of them were taken by
//! major empires, not barbarians. 13 of those came during a war that was
//! already running. On the turn before the capture, the captor had a soldier
//! within 6 tiles of the Settler in all but two of those 13. In
//! `civvis-20261105T044106Z`, Greece (power 1253 against our 547) took three
//! Settlers on t98, t99 and t100. The journal shows them stepping between open
//! tiles of lower risk ("Settler retreats from (41, 18) | its tile is
//! threatened (risk 212); stepping to (40, 19) (risk 178)"). None was sent
//! into one of our cities, the one tile a soldier cannot capture it from
//! while the city stands.
//!
//! **The gene.** While a living major at war with us has a visible land or
//! sea soldier within [`WAR_SHELTER_RADIUS`] of the Settler:
//! - a Settler standing in one of our cities holds there;
//! - a Settler in the open steps toward the nearest city of ours through the
//!   same capture-aware step the early Settler's way home uses;
//! - a Settler standing on its own target, which it can found this turn, is
//!   left to found: a city is the shelter.
//!
//! Its target is kept, so the walk resumes once the soldier is gone.
//! Barbarians are left to the existing capture rules. With the gene off,
//! nothing changes.

use super::*;

/// Tiles from the Settler inside which an enemy major's visible soldier sends
/// it to shelter. Six covered the captor on the turn before 11 of the 13
/// wartime captures. Cavalry-class units move four or five tiles, and the
/// capture is the step after the move.
pub(super) const WAR_SHELTER_RADIUS: i32 = 6;

impl AdvancedAi {
    /// The nearest visible soldier, within [`WAR_SHELTER_RADIUS`] of `at`, of
    /// a living major at war with `pid`: `(owner, distance)`.
    pub(super) fn war_shelter_threat(&self, g: &Game, pid: usize, at: Pos) -> Option<(usize, i32)> {
        g.units
            .values()
            .filter(|unit| {
                unit.owner != pid
                    && g.players
                        .get(unit.owner)
                        .is_some_and(|owner| owner.alive && !owner.is_minor && !owner.is_barbarian)
                    && g.is_at_war(pid, unit.owner)
                    && g.rules.units[unit.kind].class == "military"
                    && g.rules.units[unit.kind].domain.as_deref() != Some("air")
                    && g.unit_visible_to(unit.id, pid)
            })
            .map(|unit| (unit.owner, g.wdist(unit.pos, at)))
            .filter(|(_, distance)| *distance <= WAR_SHELTER_RADIUS)
            .min_by_key(|(owner, distance)| (*distance, *owner))
    }

    /// `settler-shelters-from-the-war`: `Some(acted)` when the gene takes this
    /// Settler's turn, `None` to leave it to the ordinary routine.
    pub(super) fn settler_shelters_from_the_war_step(
        &mut self,
        g: &mut Game,
        pid: usize,
        uid: u32,
    ) -> Option<bool> {
        if !self.settler_shelters_from_the_war {
            return None;
        }
        let current = g.units.get(&uid)?.pos;
        let (enemy, distance) = self.war_shelter_threat(g, pid, current)?;
        // Standing on its own site with the city legal: founding is shelter.
        if self.settler_targets.get(&uid) == Some(&current) && g.can_found_city(uid) {
            return None;
        }
        if let Some(city) = g
            .city_at(current)
            .filter(|city| g.cities[city].owner == pid)
        {
            think!(self.journal(), Expansion, Detail,
                   "Settler shelters in {}", g.cities[&city].name;
                   "settler-shelters-from-the-war: a soldier of player {enemy}, at war \
                    with us, stands {distance} tiles away; the city is the one tile it \
                    cannot be taken from"; current);
            return Some(true);
        }
        let shelter = g
            .player_city_ids(pid)
            .into_iter()
            .map(|cid| g.cities[&cid].pos)
            .min_by_key(|pos| (g.wdist(current, *pos), *pos))?;
        think!(self.journal(), Expansion, Detail,
               "Settler heads for shelter at {shelter:?}";
               "settler-shelters-from-the-war: a soldier of player {enemy}, at war with \
                us, stands {distance} tiles away; the nearest city of ours is {} tiles off",
               g.wdist(current, shelter); shelter);
        if self.formationless_settler_escort() && self.settlement_safety {
            self.settler_step_out_of_reach(g, pid, uid, shelter);
        } else {
            self.settler_step_toward_safe(g, pid, uid, shelter);
        }
        // Holding on safe ground when no step is safe is still this gene's
        // decision for the turn: the march toward the target is what it
        // exists to stop.
        Some(true)
    }
}

#[cfg(test)]
mod tests;
