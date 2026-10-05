//! `capture-waits-on-the-march`: a declared capture whose army is still
//! closing in is not "the objective nobody went to".
//!
//! `capture-go-or-stand-down` stands an objective down once no unit of ours
//! has come within `CAPTURE_PRESENCE_RADIUS` for `CAPTURE_GO_TURNS`
//! readings. Against a front twenty to thirty tiles from home, nobody can
//! arrive in six turns. Live King civvis-20261004T174103Z (game 68), at 3 to
//! 4 times the Netherlands' power, stood down Nijmegen after 6 turns,
//! Maastricht after 14, Amsterdam after 13, Nijmegen after 6, Batavia after
//! 27 and Amsterdam after 6, each time with the army still on the road. Every
//! stand-down re-aimed the march, and a multi-turn replay shows the siege
//! force averaging 0.85 of 9.6 members within five tiles from turn 112 to
//! 201. G68 took nothing in that war.

use super::AdvancedAi;
use crate::game::Game;

/// Standard turns a march may go without bringing our nearest land soldier
/// closer to the objective before the ledger counts the turn as forgotten.
pub(super) const CAPTURE_MARCH_PATIENCE: u32 = 3;

impl AdvancedAi {
    /// Whether our nearest land soldier to `cid` reached a new closest
    /// distance within [`CAPTURE_MARCH_PATIENCE`] standard turns. The best
    /// distance and when it was set are kept per objective tile.
    pub(super) fn capture_marching(&mut self, g: &Game, pid: usize, cid: u32) -> bool {
        if !self.capture_waits_on_the_march {
            return false;
        }
        let Some(city) = g.cities.get(&cid) else {
            return false;
        };
        let Some(nearest) = g
            .units
            .values()
            .filter(|unit| {
                let spec = &g.rules.units[unit.kind];
                unit.owner == pid
                    && spec.class == "military"
                    && spec.domain.as_deref().is_none_or(|domain| domain == "land")
            })
            .map(|unit| g.wdist(unit.pos, city.pos))
            .min()
        else {
            return false;
        };
        match self.capture_march {
            Some((pos, best, improved)) if pos == city.pos => {
                if nearest < best {
                    self.capture_march = Some((pos, nearest, g.turn));
                    true
                } else {
                    g.turn.saturating_sub(improved) <= g.standard_duration(CAPTURE_MARCH_PATIENCE)
                }
            }
            _ => {
                self.capture_march = Some((city.pos, nearest, g.turn));
                true
            }
        }
    }
}
