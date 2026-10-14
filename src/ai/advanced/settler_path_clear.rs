//! `settler-marches-alone-when-clear`: an opening Settler stops waiting for
//! its guard while no hostile can come near its road.
//!
//! The guard holds in `stacked_escort_pace` ("Settler waits for its guard",
//! "Settler falls back toward its guard") and in the civilian step ("Settler
//! waits with the guard it just called") exist because an unstacked civilian
//! is captured by any hostile that walks onto it. They hold whether or not a
//! hostile is anywhere near: a guard two tiles behind on empty ground keeps
//! the Settler parked.
//!
//! Live October 9-11 openings (Emperor and the first Immortal games): the
//! Settler took a median 5 turns at Immortal (4 at Emperor) to found 4 tiles
//! from its birth, where a three-move unit needs about 2; the last 39 games
//! logged 419 "Settler waits for its guard" turns before t80, about 10 a game.
//! Settlers captured by t60 had become rare (1 of 7 Immortal games, 4 of 50
//! Emperor). Our city count at t60 sets the production ratio at t50 against
//! the second rival: 0.27 with three cities or fewer, 0.37 with four, 0.49
//! with five, 0.53 with six or more.
//!
//! Under the gene, up to live turn [`SETTLER_ALONE_LAST_TURN`], those holds
//! give way when nothing hostile can reach the road: no visible military unit
//! of a civilization at war with us, no barbarian unit and no known barbarian
//! camp (nor ground that took a settler) within [`SETTLER_ALONE_CLEAR_RADIUS`]
//! tiles of any tile of the road to the site, and every tile of the road in
//! sight now (a road into fog keeps
//! the holds, since what is unseen there is not known to be absent). The
//! guard stays assigned and keeps following. Off: unchanged.

use super::*;

/// Tiles from the road inside which a hostile unit or barbarian camp keeps
/// the guard holds. Four covers a horseman's full move and a capture.
pub(crate) const SETTLER_ALONE_CLEAR_RADIUS: i32 = 4;

/// The least road ahead that must be in sight; the Settler's own move
/// allowance (three for Gran Colombia's) raises it. The gene re-reads the
/// road every turn, so fog farther on keeps the holds only once the Settler
/// is close enough to step into it.
pub(crate) const SETTLER_ALONE_SIGHT_AHEAD: i32 = 2;

/// The last live turn the gene acts on: the opening expansion window.
pub(crate) const SETTLER_ALONE_LAST_TURN: u32 = 80;

impl AdvancedAi {
    /// Under the gene, whether settler `uid` may march to its site without
    /// waiting for its guard: an opening turn, a known site, the next
    /// [`SETTLER_ALONE_SIGHT_AHEAD`] tiles of a shortest road to it in sight,
    /// and no hostile unit, barbarian camp or capture scar within
    /// [`SETTLER_ALONE_CLEAR_RADIUS`] tiles of that road.
    ///
    /// "Within the radius of some tile of the road" is tested as
    /// `d(h, here) + d(h, site) <= d(here, site) + 2 * radius`, which holds
    /// for every tile within the radius of any shortest road and so never
    /// clears a road a hostile could reach.
    pub(super) fn settler_path_clear(&self, g: &Game, pid: usize, uid: u32) -> Option<(Pos, i32)> {
        if !self.settler_marches_alone_when_clear || g.turn > SETTLER_ALONE_LAST_TURN {
            return None;
        }
        let here = g.units.get(&uid)?.pos;
        let site = *self.settler_targets.get(&uid)?;
        let ahead = (g.unit_max_moves(uid).ceil() as i32).max(SETTLER_ALONE_SIGHT_AHEAD);
        self.road_is_clear(
            g,
            pid,
            here,
            site,
            ahead,
            &self.battlefront_visibility(g, pid),
        )
        .then(|| (site, g.wdist(here, site)))
    }

    /// Under the gene, journal and release a guard hold on a clear road:
    /// `true` when the Settler should march instead of holding. The guard
    /// stays assigned and keeps following.
    pub(super) fn settler_marches_alone(
        &mut self,
        g: &Game,
        pid: usize,
        uid: u32,
        waited: u8,
    ) -> bool {
        let Some((site, span)) = self.settler_path_clear(g, pid, uid) else {
            return false;
        };
        let here = g.units[&uid].pos;
        think!(self.journal(), Expansion, Detail, "Settler marches alone: path clear";
               "settler-marches-alone-when-clear: no hostile unit, barbarian camp or capture \
                scar within {} tiles of the {span}-tile road to {site:?}, and this turn's \
                steps are in sight; {waited} turn(s) waited for the guard, which keeps \
                following",
               SETTLER_ALONE_CLEAR_RADIUS; here);
        self.guard_wait.remove(&uid);
        true
    }

    /// The road test of [`Self::settler_path_clear`], on an explicit frame.
    pub(super) fn road_is_clear(
        &self,
        g: &Game,
        pid: usize,
        here: Pos,
        site: Pos,
        ahead: i32,
        visible: &crate::world::TileBits,
    ) -> bool {
        let span = g.wdist(here, site);
        let reach = span + 2 * SETTLER_ALONE_CLEAR_RADIUS;
        let near_road = |pos: Pos| g.wdist(pos, here) + g.wdist(pos, site) <= reach;
        // Every tile of a shortest road that the Settler can reach this turn
        // is in sight. The road beyond is read again next turn, from nearer.
        let road_seen = g.map.tiles.values().all(|tile| {
            g.wdist(tile.pos, here) + g.wdist(tile.pos, site) > span
                || g.wdist(tile.pos, here) > ahead
                || g.sees(visible, tile.pos)
        });
        if !road_seen {
            return false;
        }
        let hostile_near = g.units.values().any(|unit| {
            unit.owner != pid
                && g.is_at_war(pid, unit.owner)
                && (g.players[unit.owner].is_barbarian
                    || (g.rules.units[unit.kind].class == "military"
                        && g.rules.units[unit.kind].domain.as_deref() != Some("air")))
                && g.sees(visible, unit.pos)
                && near_road(unit.pos)
        });
        // A known camp is a raider not yet out of the fog, and the ground that
        // took a settler is not quiet because nothing stands on it today
        // (the same two exceptions the escort's own threat gate makes).
        let camp_near = g.barb_camps.keys().any(|camp| near_road(*camp))
            || g.map.tiles.values().any(|tile| {
                tile.improvement.as_deref() == Some("barbarian_camp") && near_road(tile.pos)
            });
        let scarred = self
            .settler_capture_scars
            .keys()
            .any(|scar| near_road(*scar));
        !hostile_near && !camp_near && !scarred
    }
}

#[cfg(test)]
mod tests;
