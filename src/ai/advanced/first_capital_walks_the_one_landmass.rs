//! `first-capital-walks-the-one-landmass`: on a map with one landmass,
//! `first-capital-before-the-walls` counts uncharted ground as land when it
//! asks whether a walk reaches the target capital, and the nearest free
//! scout charts the fog on that walk.
//!
//! ## Why
//!
//! Live Deity Macedon, four majors on a Tiny Pangaea at Online speed, game
//! civvis-20261111T040245Z: the first-meeting capital exchange charted
//! Washington on turn 11, 12 tiles from our capital and behind no walls. At
//! every note from turn 12 to turn 62, the last before the turn-63 naming
//! deadline, the opening refused it: "not Washington | 12 tiles from our
//! capital, walls 0: no charted land walk reaches their capital". The
//! exchange reveals the capital and its ring, not the ground between, and
//! nothing sent a unit to chart that ground. The best target the seat saw all
//! game, an unwalled Deity capital twelve tiles out, was refused for the
//! whole window.
//!
//! The charted walk exists because the live mirror reads uncharted ground as
//! default land, so a capital across a strait would otherwise pass. A map
//! that promises one landmass has no such strait between two capitals.
//!
//! ## What the gene changes
//!
//! Only while `first-capital-before-the-walls` plays (the Domination lane,
//! with `early-conquest-opening`) and only on a map script
//! [`map_script_is_one_landmass`] names:
//!
//! 1. **The walk** ([`AdvancedAi::first_capital_walk_is_charted`]) counts an
//!    uncharted tile inside its bound as ground. It tests neither terrain nor
//!    owner there, since we know neither. A charted tile keeps every test
//!    (water, impassable, the borders rule), and the walk still starts at
//!    home, stays within the bound and ends beside the capital, so a charted
//!    sea or mountain range across the way still refuses the capital.
//! 2. **The journal**: when the opening is named on a capital that only such
//!    a walk reaches, one line at naming says how many uncharted tiles the
//!    walk crosses ([`AdvancedAi::first_capital_note_the_landmass_walk`]).
//! 3. **The corridor**: while the opening stands undeclared on that capital
//!    (or, before it is named, while the window is open and the scan would
//!    name it) and no charted walk reaches it, the free scout nearest the
//!    first uncharted tile on the shortest walk heads there
//!    ([`AdvancedAi::first_capital_corridor_goal`]), so the ground the force
//!    will march over is charted before it marches. The capital-hunt
//!    assignment of `find-the-capital` still comes first.
//!
//! On every other map script the strict charted walk stands and the scouts
//! keep their sweep. Off, or without the parent gene, nothing changes.

use super::AdvancedAi;
use crate::game::Game;
use crate::setup::MapScript;
use crate::think;
use crate::Pos;

use super::first_capital_before_the_walls::FIRST_CAPITAL_NAMING_DEADLINE;

/// Whether `script` promises one landmass that every major starts on.
///
/// - **Pangaea**: "One connected supercontinent surrounded by ocean"
///   (`setup::CIV6_MAP_SCRIPTS`); Civilization VI's own: "One massive
///   landmass with surrounding islands" (`LOC_MAP_PANGAEA_DESCRIPTION`).
///   The offshore islands hold no major's start.
/// - **Land Only**: "one unbroken world with a scatter of inland seas"; 95%
///   of the world is ground.
/// - **Lakes**: "A world of land, broken up by lakes and inland seas instead
///   of oceans"; Civilization VI's: "A world without oceans, only lakes and
///   inland seas". 81% ground.
///
/// Not **Inland Sea**, though its ring of land is connected: its one sea
/// fills the middle of the map, so the straight walk across fog between two
/// capitals on opposite shores is exactly the crossing the charted walk is
/// there to refuse, and the land walk around it is far past the bound. Not
/// Tennis Ball (two lobes split by a sea seam), the Grand Canals (canals cut
/// the ground), any multi-continent, island or Earth map, or a Tactics map.
pub(crate) const fn map_script_is_one_landmass(script: MapScript) -> bool {
    matches!(
        script,
        MapScript::Pangaea | MapScript::LandOnly | MapScript::Lakes
    )
}

/// The name a journal line gives a one-landmass map.
fn one_landmass_name(script: MapScript) -> &'static str {
    match script {
        MapScript::LandOnly => "Land Only",
        MapScript::Lakes => "Lakes",
        _ => "Pangaea",
    }
}

impl AdvancedAi {
    /// Whether the first capital's walk counts uncharted ground as land
    /// here: the flag, `first-capital-before-the-walls` playing, and a map
    /// with one landmass.
    pub(super) fn first_capital_walks_the_one_landmass_on(&self, g: &Game) -> bool {
        self.first_capital_walks_the_one_landmass
            && self.first_capital_active()
            && map_script_is_one_landmass(g.map_script)
    }

    /// The uncharted tiles, home first, on the shortest walk from our
    /// capital to `city` of `rival`'s when no charted walk reaches it and the
    /// gene's walk does. Empty with the gene off or on another map, when a
    /// charted walk reaches the city, or when no walk does.
    pub(super) fn first_capital_uncharted_corridor(
        &self,
        g: &Game,
        pid: usize,
        rival: usize,
        city: u32,
    ) -> Vec<Pos> {
        if !self.first_capital_walks_the_one_landmass_on(g) {
            return Vec::new();
        }
        let (Some(home), Some(target)) = (
            Self::first_capital_home(g, pid),
            g.cities.get(&city).map(|city| city.pos),
        ) else {
            return Vec::new();
        };
        if Self::first_capital_walk(g, pid, rival, home, target, false).is_some() {
            return Vec::new();
        }
        let explored = &g.players[pid].explored;
        Self::first_capital_walk(g, pid, rival, home, target, true)
            .map(|walk| {
                walk.into_iter()
                    .filter(|pos| !explored.contains(pos))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// At naming: journal that the walk to the named capital crosses
    /// uncharted ground, and how much. Silent when it does not.
    pub(super) fn first_capital_note_the_landmass_walk(
        &self,
        g: &Game,
        pid: usize,
        rival: usize,
        city: u32,
    ) {
        let corridor = self.first_capital_uncharted_corridor(g, pid, rival, city);
        let Some(first) = corridor.first().copied() else {
            return;
        };
        think!(self.journal(), Military, Detail,
               "First capital before the walls: {} over uncharted ground", g.cities[&city].name;
               "no charted land walk reaches it; the walk crosses {} uncharted tile{}; a {} map \
                has one landmass",
               corridor.len(), if corridor.len() == 1 { "" } else { "s" },
               one_landmass_name(g.map_script);
               first);
    }

    /// The capital whose corridor the scouts chart: the target of an
    /// opening that stands undeclared and has taken nothing, or, before one
    /// is named, the capital the scan would name while the window is open.
    fn first_capital_corridor_target(&self, g: &Game, pid: usize) -> Option<(usize, u32)> {
        if let Some(opening) = self.conquest_opening.as_ref() {
            return (opening.declared.is_none() && opening.taken == 0)
                .then_some((opening.target, opening.city));
        }
        if self.conquest_closed || g.turn >= g.standard_duration(FIRST_CAPITAL_NAMING_DEADLINE) {
            return None;
        }
        self.first_capital_target(g, pid)
    }

    /// The goal of the free scout `uid` while the first capital's walk
    /// crosses uncharted ground: the first uncharted tile on the walk it can
    /// enter and has not given up on, if `uid` is the free scout nearest
    /// that tile. `None` otherwise, and always with the gene off.
    pub(super) fn first_capital_corridor_goal(
        &self,
        g: &Game,
        pid: usize,
        uid: u32,
    ) -> Option<Pos> {
        if !self.first_capital_walks_the_one_landmass_on(g) {
            return None;
        }
        let (rival, city) = self.first_capital_corridor_target(g, pid)?;
        let corridor = self.first_capital_uncharted_corridor(g, pid, rival, city);
        if corridor.is_empty() {
            return None;
        }
        let goal = {
            let dead = self.base.explore_dead.borrow();
            let given_up = |pos: Pos| {
                dead.get(&uid)
                    .and_then(|goals| goals.get(&pos))
                    .is_some_and(|expiry| *expiry > g.turn)
            };
            corridor
                .into_iter()
                .find(|pos| g.unit_can_traverse(uid, *pos) && !given_up(*pos))?
        };
        let nearest = g
            .player_unit_ids(pid)
            .into_iter()
            .filter(|other| self.distance_scout_available(g, pid, *other))
            .min_by_key(|other| (g.wdist(g.units[other].pos, goal), *other));
        if nearest != Some(uid) {
            return None;
        }
        let fresh = self
            .base
            .explore_goal
            .borrow()
            .get(&uid)
            .is_none_or(|(held, _)| *held != goal);
        if fresh {
            think!(self.journal(), Military, Detail,
                   "{} {uid} charts the walk to {}", g.units[&uid].kind, g.cities[&city].name;
                   "the first capital's walk crosses uncharted ground on a map with one \
                    landmass; the nearest free scout charts it before the force marches";
                   goal);
        }
        Some(goal)
    }
}

#[cfg(test)]
mod tests;
