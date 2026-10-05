//! `find-the-capital`: a Domination plan cannot finish while a rival's
//! original capital it needs has never been seen.
//!
//! ★★★ THE LAST CAPITAL WAS NEVER ON THE BOARD. Live King
//! civvis-20261005T081917Z (game 112) took Amsterdam at turn 73 and Xanadu at
//! 118 (retaken at 138), holding two of three rival original capitals, and
//! lost to Kongo's Diplomatic Victory at 202 without once seeing Kongo's
//! capital. By 196 eight Kongo cities were known and none was its original
//! capital: their revealed ground ringed a fogged pocket in the middle of the
//! cluster. The lead scout stood at Kongo's sealed border
//! (`scout_distance::rival_lead_goal`) and the passage purchase that opens it
//! held 47 times as `border_buy_hold:treasury` — not for want of Gold (201
//! at turn 67) but because `Game::passage_gold_value` books Open Borders at
//! 28 + 0.35 × tourism a turn, under the lane's 30 Gold minimum ask, for
//! a seat with no tourism. Nothing else sends a unit inside.
//!
//! Under the gene:
//! - the passage to such a rival, while we are at peace with it, is worth
//!   [`FIND_CAPITAL_PASSAGE_GOLD`] to the border-buy lane, which then asks;
//! - once that rival's ground is open to us (Open Borders granted, or war)
//!   the Board carries one Recon row aimed at the fog nearest the middle of
//!   their known ground, which a Scout or a fast land unit may take.
//!
//! "Never seen" is the seat's own knowledge: no city on the board and none in
//! `remembered_cities` is that rival's original capital (`is_capital` with
//! the rival as `original_owner`). A capital held by the Free Cities before
//! we ever saw it is not exported as a city at all — the live export carries
//! majors' and city-states' cities only, the Free Cities' only as owned
//! plots — so it reads as unseen here too. The row then still points at the
//! fog beside that rival's ground, which is where such a city would stand.
use super::objective_board::{ForceNeed, Objective, ObjectiveKey, ObjectiveKind, RowState};
use super::{AdvancedAi, Game, VictoryTarget};
use crate::Pos;

/// What the passage to a rival whose needed capital we have never seen is
/// worth, in Gold. The border-buy lane still bounds its ask by the treasury
/// (less its reserve) and its own cap, so this only lifts the book above the
/// lane's minimum and to a price a reluctant AI may take.
pub const FIND_CAPITAL_PASSAGE_GOLD: f64 = 150.0;
/// The hunt row's value, against a sector Recon row's (a Scout's cost):
/// finding the last capital is worth more than any one sector.
const FIND_CAPITAL_ROW_VALUE: f64 = 300.0;
/// Fog this close to a rival's known ground can hold one of its cities.
const FIND_CAPITAL_FOG_REACH: i32 = 4;
/// Unattributed foreign ground this close to a rival's known city is read as
/// that rival's; a city owns plots out to five rings.
const FIND_CAPITAL_GROUND_REACH: i32 = 6;

impl AdvancedAi {
    /// Met major rivals whose original capital a Domination plan still needs
    /// and that this seat has never seen. Empty with the gene off or on any
    /// other lane.
    pub(super) fn unfound_capital_rivals(&self, g: &Game, pid: usize) -> Vec<usize> {
        if !self.find_the_capital
            || self.active_victory_target(g) != Some(VictoryTarget::Domination)
        {
            return Vec::new();
        }
        g.players
            .iter()
            .filter(|rival| {
                rival.id != pid
                    && rival.alive
                    && !rival.is_minor
                    && !rival.is_barbarian
                    && !rival.is_free_city
                    && !g.same_team(pid, rival.id)
                    && g.has_met(pid, rival.id)
            })
            .map(|rival| rival.id)
            .filter(|rival| !Self::original_capital_known(g, pid, *rival))
            .collect()
    }

    /// Whether this seat knows where `rival`'s original capital stands, on
    /// the board or in its memory of cities it has seen.
    fn original_capital_known(g: &Game, pid: usize, rival: usize) -> bool {
        g.cities
            .values()
            .any(|city| city.is_capital && city.original_owner == rival)
            || g.players.get(pid).is_some_and(|seat| {
                seat.remembered_cities
                    .values()
                    .any(|city| city.is_capital && city.original_owner == rival)
            })
    }

    /// The passage price the border-buy lane may pay `seat` for Open Borders:
    /// [`FIND_CAPITAL_PASSAGE_GOLD`] while we are at peace with a rival whose
    /// needed capital we have never seen, `None` otherwise (the lane keeps
    /// the engine's own book).
    pub fn find_the_capital_passage_gold(&self, g: &Game, pid: usize, seat: usize) -> Option<f64> {
        (!g.is_at_war(pid, seat) && self.unfound_capital_rivals(g, pid).contains(&seat))
            .then_some(FIND_CAPITAL_PASSAGE_GOLD)
    }

    /// Whether our land units may walk into `rival`'s ground now: we are at
    /// war with them, or they granted us Open Borders that still run.
    fn rival_ground_open(g: &Game, pid: usize, rival: usize) -> bool {
        g.is_at_war(pid, rival)
            || g.players
                .get(rival)
                .and_then(|player| player.open_borders_until.get(&pid))
                .is_some_and(|until| *until > g.turn)
    }

    /// The tiles we know `rival` holds: its known cities, the plots they own,
    /// and unattributed major ground (`Game::unseen_major_borders`) near one
    /// of those cities — or all of it when no city of theirs is known.
    fn rival_known_ground(g: &Game, rival: usize) -> Vec<Pos> {
        let cities: Vec<Pos> = g
            .cities
            .values()
            .filter(|city| city.owner == rival)
            .map(|city| city.pos)
            .collect();
        let mut ground: Vec<Pos> = cities.clone();
        for (pos, tile) in &g.map.tiles {
            if tile
                .owner_city
                .and_then(|cid| g.cities.get(&cid))
                .is_some_and(|city| city.owner == rival)
            {
                ground.push(*pos);
            }
        }
        for pos in &g.unseen_major_borders {
            if cities.is_empty()
                || cities
                    .iter()
                    .any(|city| g.wdist(*city, *pos) <= FIND_CAPITAL_GROUND_REACH)
            {
                ground.push(*pos);
            }
        }
        ground.sort_unstable();
        ground.dedup();
        ground
    }

    /// Where to look for `rival`'s capital: the unexplored tile nearest the
    /// middle (medoid) of its known ground, among fog within
    /// [`FIND_CAPITAL_FOG_REACH`] of that ground. A first city is founded
    /// before the others and the rest grow around it, so the middle of a
    /// cluster is where an unseen capital most often stands; in game 112 the
    /// last fog inside Kongo's eight known cities was a pocket in their
    /// middle.
    pub(super) fn find_capital_goal(&self, g: &Game, pid: usize, rival: usize) -> Option<Pos> {
        let ground = Self::rival_known_ground(g, rival);
        let centre = ground.iter().copied().min_by_key(|pos| {
            (
                ground
                    .iter()
                    .map(|other| g.wdist(*pos, *other))
                    .sum::<i32>(),
                *pos,
            )
        })?;
        let explored = &g.players.get(pid)?.explored;
        g.map
            .tiles
            .keys()
            .copied()
            .filter(|pos| !explored.contains(pos))
            .filter(|pos| {
                ground
                    .iter()
                    .any(|known| g.wdist(*known, *pos) <= FIND_CAPITAL_FOG_REACH)
            })
            .min_by_key(|pos| (g.wdist(centre, *pos), *pos))
    }

    /// The Board's hunt row: one Recon row for the nearest rival (to our
    /// cities) whose needed capital is unseen and whose ground is open to
    /// us, aimed at [`Self::find_capital_goal`].
    pub(super) fn find_capital_row(&self, g: &Game, pid: usize) -> Option<Objective> {
        let ours: Vec<Pos> = g
            .player_city_ids(pid)
            .into_iter()
            .map(|cid| g.cities[&cid].pos)
            .collect();
        self.unfound_capital_rivals(g, pid)
            .into_iter()
            .filter(|rival| Self::rival_ground_open(g, pid, *rival))
            .filter_map(|rival| {
                let goal = self.find_capital_goal(g, pid, rival)?;
                let home = ours
                    .iter()
                    .map(|pos| g.wdist(*pos, goal))
                    .min()
                    .unwrap_or(i32::MAX);
                Some((home, rival, goal))
            })
            .min()
            .map(|(_, rival, goal)| Objective {
                kind: ObjectiveKind::Recon,
                key: ObjectiveKey::FindCapital(rival),
                at: goal,
                value: FIND_CAPITAL_ROW_VALUE,
                requirement: ForceNeed {
                    strength: 0.0,
                    melee: 0,
                    ranged: 0,
                    siege: 0,
                    bodies: 1,
                },
                deadline: None,
                state: RowState::Open,
                depends_on: None,
                land: true,
                sea: false,
                label: format!("{}'s capital", g.players[rival].civ),
                urgent: false,
            })
    }
}

#[cfg(test)]
mod tests;
