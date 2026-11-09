//! `guru-walks-to-the-wounded`: a Guru goes where its heals are wanted.
//!
//! **Why (live games of 2026-11-09).** With nothing to heal this turn, a
//! Guru's step walks toward the nearest rival religious unit, because that is
//! the last leg of `advanced_religious_step` for any unit with no spread to
//! make. 21 Gurus in 5 games stood within 4 tiles of a wounded friend for 80
//! Guru-turns and beside one for only 13, and rival Apostles killed 8 of the
//! 16 Gurus they attacked. A Guru cannot start a fight, so walking at a rival
//! only hands it a free kill.

use super::AdvancedAi;
use crate::game::Game;
use crate::Pos;

impl AdvancedAi {
    /// Where this Guru walks instead of at the nearest rival: the nearest
    /// wounded unit of its faith, else the nearest unit of its faith it could
    /// heal later, else the Holy City. A Guru with no heals left goes straight
    /// to the Holy City. `None` with the gene off, for any other unit, or
    /// when none of those exists.
    pub(super) fn guru_walk_target(&self, g: &Game, pid: usize, uid: u32) -> Option<Pos> {
        if !self.guru_walks_to_the_wounded {
            return None;
        }
        let guru = g.units.get(&uid)?;
        if guru.kind != "guru" {
            return None;
        }
        let holy_city = g.players[pid]
            .holy_city
            .and_then(|city| g.cities.get(&city))
            .map(|city| city.pos);
        if guru.charges <= 0 {
            return holy_city;
        }
        let nearest = |wounded_only: bool| {
            g.units
                .values()
                .filter(|other| {
                    other.owner == pid
                        && other.id != uid
                        && other.kind != "guru"
                        && other.religion == guru.religion
                        && g.rules.units[other.kind].class == "religious"
                        && (!wounded_only || other.hp < 100)
                })
                .min_by_key(|other| (g.wdist(guru.pos, other.pos), other.id))
                .map(|other| other.pos)
        };
        nearest(true).or_else(|| nearest(false)).or(holy_city)
    }
}

#[cfg(test)]
mod tests;
