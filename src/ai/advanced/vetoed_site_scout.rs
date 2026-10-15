//! `vetoed-site-sends-a-scout`: a good settle site refused because a rival
//! city may be hidden beside it sends a scout to look, and the refusal ends
//! once the ground that could hide the city has been seen.
//!
//! ## The live evidence
//!
//! **G030910Z** (`civvis-20261015T030910Z`, Emperor, pin 73d32f7fa). The sites
//! at axial (15, 16), (15, 17) and (15, 18), worth about 100–133, were refused
//! seven times between t50 and t105 with *"it lies within five tiles of a
//! rival border whose city may be hidden — the forecast cannot price that
//! Loyalty pressure"*. Nothing was sent to look, although some ten scouts were
//! out exploring elsewhere (`"scout N explores beyond home"`). At t74 (15, 16)
//! ranked first for Popayán's Settler and was vetoed again. The Settler then
//! founded Cuenca on snow at axial (21, 3): worth 69.5, its first ring snow,
//! tundra, a mountain and ice, population 2 at t100 and 4 at t150.
//!
//! The vetoing plot was Georgia's at offset (22, 21). Its nearest known
//! Georgian city, Kutaisi, stands exactly five tiles away. The mirror marks
//! that outer ownership ring as unresolved (`CIV6_CITY_OWNERSHIP_REACH` in
//! `mirror.rs`): the export names only the owning player, so a nearer city in
//! the fog could own the plot instead. 28 of the 61 tiles within four of the
//! plot had never been revealed. Exploring can therefore settle the question
//! either way, but the mirror's rule never lets go of a plot at exactly five,
//! so the veto was permanent even after a scout had passed.
//!
//! **Census** (127 live Emperor runs, 10-14 07:30Z to 10-15):
//! - 549 such refusals in 64 runs, of 337 distinct sites. 29 sites were
//!   refused over a span of at least 20 turns.
//! - Of 631 cities we founded outside the capital, 66 (10%) stand within four
//!   rows of a pole. Their sites were worth a median 46 against 135 elsewhere,
//!   and they reached a median population of 4 at t150 against 8.
//!
//! ## What the gene does
//!
//! - **Records a veto.** When the target search, the exhaustion search or the
//!   settler-site gate drops a site for an unresolved rival border, and the
//!   site is worth at least [`VETOED_SITE_WORTH_FLOOR`], the site is kept for
//!   [`VETOED_SITE_MEMORY_STANDARD`] standard turns from its last refusal. The
//!   hunt for one site stops after [`VETOED_SITE_SCOUT_CAP_STANDARD`] standard
//!   turns, or when a city stands within three tiles of it.
//! - **Sends a scout.** Of the free recon units (`distance_scout_available`),
//!   the one nearest the fog holds the job. Its explorer goal becomes the
//!   nearest unexplored land tile within [`VETOED_BORDER_RESOLVE_REACH`] of a
//!   vetoing plot. Routing, threat checks and dead-goal retirement are the
//!   explorer's own. With no free recon unit, the Objective Board carries a
//!   Recon row for the best site, which any land body other than a siege gun
//!   may take.
//! - **Resolves the plot.** An unresolved plot whose land within
//!   [`VETOED_BORDER_RESOLVE_REACH`] has all been explored no longer vetoes a
//!   site. A city that could own the plot would stand inside that disk, so it
//!   is either on the board, and the Loyalty forecast prices it, or it does
//!   not exist. The fog guard `beyond_loyalty_reach` is unchanged.
//!
//! Off (the default), every path is unchanged.
use super::objective_board::{ForceNeed, Objective, ObjectiveKey, ObjectiveKind, RowState};
use super::{AdvancedAi, UNRESOLVED_MAJOR_BORDER_RADIUS};
use crate::game::Game;
use crate::think;
use crate::Pos;
use std::collections::BTreeMap;

/// The least a refused site may be worth to be worth a scout. Founded sites
/// on the 10-14 Emperor census sit at a median 126; a quarter are under 89.
pub const VETOED_SITE_WORTH_FLOOR: f64 = 90.0;

/// Standard turns a refusal is remembered after its last repeat (ten on
/// Online). A site the walker keeps asking about keeps its record.
pub const VETOED_SITE_MEMORY_STANDARD: u32 = 20;

/// Standard turns from a site's first refusal after which no unit is sent to
/// it (thirty on Online). Ground behind a sealed border may never be seen.
pub const VETOED_SITE_SCOUT_CAP_STANDARD: u32 = 60;

/// How far from a vetoing plot a city that owns it can stand: Civilization
/// VI's ownership reach (`CIV6_CITY_OWNERSHIP_REACH` in `mirror.rs`).
pub const VETOED_BORDER_RESOLVE_REACH: i32 = 5;

/// The Board row's value with no Scout to send, against a sector Recon row's
/// (a Scout's cost, 30).
const VETOED_BORDER_ROW_VALUE: f64 = 60.0;

/// One refused site: what it was worth and when it was refused.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct VetoedSite {
    pub(super) worth: f64,
    pub(super) first: u32,
    pub(super) last: u32,
}

impl AdvancedAi {
    /// Whether the unresolved-border veto refuses `site`. With the gene off,
    /// this is `beside_unresolved_major_border` exactly. With it on, a plot
    /// whose surroundings have all been explored is no longer unresolved.
    pub(super) fn unresolved_border_vetoes(&self, g: &Game, pid: usize, site: Pos) -> bool {
        if !self.vetoed_site_sends_a_scout {
            return Self::beside_unresolved_major_border(g, site);
        }
        !Self::open_border_plots_beside(g, pid, site).is_empty()
    }

    /// The unresolved major-border plots within the veto's radius of `site`
    /// that could still hide a city: some land within
    /// [`VETOED_BORDER_RESOLVE_REACH`] of the plot is unexplored.
    pub(super) fn open_border_plots_beside(g: &Game, pid: usize, site: Pos) -> Vec<Pos> {
        if g.unseen_major_borders.is_empty() {
            return Vec::new();
        }
        g.wdisk(site, UNRESOLVED_MAJOR_BORDER_RADIUS)
            .into_iter()
            .filter(|pos| g.unseen_major_borders.contains(pos))
            .filter(|plot| !Self::hiding_ground(g, pid, *plot).is_empty())
            .collect()
    }

    /// Unexplored land within [`VETOED_BORDER_RESOLVE_REACH`] of `plot`:
    /// where a city owning it could still stand unseen.
    fn hiding_ground(g: &Game, pid: usize, plot: Pos) -> Vec<Pos> {
        let Some(seat) = g.players.get(pid) else {
            return Vec::new();
        };
        g.wdisk(plot, VETOED_BORDER_RESOLVE_REACH)
            .into_iter()
            .filter(|pos| {
                !seat.explored.contains(pos)
                    && g.map.get(*pos).is_some_and(|tile| !g.rules.is_water(tile))
            })
            .collect()
    }

    /// Remember that `site`, worth `worth`, was refused for an unresolved
    /// border this turn. A no-op with the gene off or under the floor.
    pub(super) fn note_vetoed_site(&self, g: &Game, site: Pos, worth: f64) {
        if !self.vetoed_site_sends_a_scout || worth < VETOED_SITE_WORTH_FLOOR {
            return;
        }
        let mut sites = self.vetoed_sites.borrow_mut();
        match sites.get_mut(&site) {
            Some(record) => {
                record.worth = record.worth.max(worth);
                record.last = g.turn;
            }
            None => {
                sites.insert(
                    site,
                    VetoedSite {
                        worth,
                        first: g.turn,
                        last: g.turn,
                    },
                );
                think!(self.journal(), Expansion, Detail,
                       "A hidden rival city may stand beside {site:?}";
                       "the site is worth {worth:.1} and the Loyalty guard refuses it for a rival \
                        border whose city we have never seen; a scout will look"; site);
            }
        }
    }

    /// The remembered sites still worth a look, best first, each with the
    /// plots that still veto it. Expired records are dropped.
    pub(super) fn live_vetoed_sites(&self, g: &Game, pid: usize) -> Vec<(Pos, VetoedSite, Vec<Pos>)> {
        if !self.vetoed_site_sends_a_scout {
            return Vec::new();
        }
        let memory = g.standard_duration(VETOED_SITE_MEMORY_STANDARD);
        let cap = g.standard_duration(VETOED_SITE_SCOUT_CAP_STANDARD);
        let mut sites = self.vetoed_sites.borrow_mut();
        sites.retain(|_, record| {
            g.turn.saturating_sub(record.last) <= memory
                && g.turn.saturating_sub(record.first) <= cap
        });
        let mut live: Vec<(Pos, VetoedSite, Vec<Pos>)> = sites
            .iter()
            .filter(|(site, _)| !g.cities.values().any(|city| g.wdist(city.pos, **site) < 4))
            .filter_map(|(site, record)| {
                let plots = Self::open_border_plots_beside(g, pid, *site);
                (!plots.is_empty()).then_some((*site, *record, plots))
            })
            .collect();
        live.sort_by(|a, b| b.1.worth.total_cmp(&a.1.worth).then(a.0.cmp(&b.0)));
        live
    }

    /// The fog a scout should open for the remembered sites: each unexplored
    /// land tile that could hide a vetoing city, with the best site it serves.
    fn vetoed_site_fog(&self, g: &Game, pid: usize) -> BTreeMap<Pos, Pos> {
        let mut fog = BTreeMap::new();
        for (site, _, plots) in self.live_vetoed_sites(g, pid) {
            for plot in plots {
                for pos in Self::hiding_ground(g, pid, plot) {
                    fog.entry(pos).or_insert(site);
                }
            }
        }
        fog
    }

    /// The nearest fog tile `scout` can head for, with the distance and site.
    fn vetoed_site_goal_for(
        &self,
        g: &Game,
        scout: u32,
        fog: &BTreeMap<Pos, Pos>,
    ) -> Option<(i32, Pos, Pos)> {
        let dead: Vec<Pos> = self
            .base
            .explore_dead
            .borrow()
            .get(&scout)
            .map(|dead| {
                dead.iter()
                    .filter(|(_, expiry)| **expiry > g.turn)
                    .map(|(pos, _)| *pos)
                    .collect()
            })
            .unwrap_or_default();
        let from = g.units.get(&scout)?.pos;
        fog.iter()
            .filter(|(pos, _)| !dead.contains(pos) && g.unit_can_traverse(scout, **pos))
            .map(|(pos, site)| (g.wdist(from, *pos), *pos, *site))
            .min()
    }

    /// `uid`'s goal under the gene, with the site it looks for: the nearest
    /// fog that could hide a vetoing city, when `uid` is the free recon unit
    /// nearest that fog. `None` for every other unit and with the gene off.
    pub(super) fn vetoed_site_scout_goal(&self, g: &Game, pid: usize, uid: u32) -> Option<(Pos, Pos)> {
        if !self.vetoed_site_sends_a_scout || g.unseen_major_borders.is_empty() {
            return None;
        }
        let fog = self.vetoed_site_fog(g, pid);
        if fog.is_empty() {
            return None;
        }
        let (_, chosen) = g
            .player_unit_ids(pid)
            .into_iter()
            .filter(|other| self.distance_scout_available(g, pid, *other))
            .filter_map(|other| {
                self.vetoed_site_goal_for(g, other, &fog)
                    .map(|(distance, _, _)| (distance, other))
            })
            .min()?;
        if chosen != uid {
            return None;
        }
        self.vetoed_site_goal_for(g, uid, &fog)
            .map(|(_, goal, site)| (goal, site))
    }

    /// The Board's row when no free recon unit can go: one Recon row for the
    /// best remembered site, aimed at the fog nearest the plot that vetoes it.
    pub(super) fn vetoed_border_row(&self, g: &Game, pid: usize) -> Option<Objective> {
        if !self.vetoed_site_sends_a_scout
            || g
                .player_unit_ids(pid)
                .into_iter()
                .any(|uid| self.distance_scout_available(g, pid, uid))
        {
            return None;
        }
        let (site, record, plots) = self.live_vetoed_sites(g, pid).into_iter().next()?;
        let goal = plots
            .iter()
            .flat_map(|plot| {
                Self::hiding_ground(g, pid, *plot)
                    .into_iter()
                    .map(move |pos| (g.wdist(*plot, pos), pos))
            })
            .min()
            .map(|(_, pos)| pos)?;
        think!(self.journal(), Military, Detail,
               "Looking behind the border beside {site:?}";
               "the site is worth {:.1} and refused for a rival city that may be hidden; with no \
                Scout free, a land unit takes the look at {goal:?}", record.worth; goal);
        Some(Objective {
            kind: ObjectiveKind::Recon,
            key: ObjectiveKey::VetoedBorder(site),
            at: goal,
            value: VETOED_BORDER_ROW_VALUE,
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
            label: format!("the border beside {site:?}"),
            urgent: false,
        })
    }
}

#[cfg(test)]
mod tests;
