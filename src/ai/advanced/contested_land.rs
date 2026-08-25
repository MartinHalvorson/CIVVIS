//! Contested land first: while the army can hold it, the ground between us
//! and the nearest neighbours is claimed before the ground behind us, and
//! the cities that claim it are walled and garrisoned.
//!
//! ★★★ THE SITE MODEL RUNS AWAY FROM EVERY NEIGHBOUR. Three terms price a
//! settle site by its distance from a rival, and all three point the same
//! way: `settlement_static_value_uncached` takes six points a tile off any
//! site under six tiles from a foreign city, `foreign_border_pressure` takes
//! four a tile (capped at forty) for rival-owned ground within three, and
//! `settlement_safety_penalty` adds the isolation term on top. Nothing pays
//! for the one thing a site between two empires has that a site behind us
//! does not: a rival's Settler is walking toward it. The land at our back is
//! ours whenever we want it; the land between us is whoever's settler
//! arrives first — and the live seat, priced this way, forward-settled only
//! when every other site was worse, then lost the city because nothing
//! defended it (memory `civvis-civ6-settler-lane`, T021044Z/T040537Z/
//! T045316Z: three forward cities, three wars, six cities lost — with no
//! army behind the claim). The operator's rule (2026-08-25): *more
//! aggressively claim land in the direction of our nearest neighbours, so
//! long as we have a decent military; claim this more contested land first
//! and defend it, and then claim the uncontested land when the contested
//! land runs out.*
//!
//! What the gene does, all of it inert while the flag is off:
//!
//! 1. **Fronts.** A front is a met, living major's city within
//!    [`CONTESTED_LAND_REACH`] of one of ours (a team-mate is no rival; a
//!    major we are at war with is the war planner's business, not the
//!    Settler's), paired with our city nearest it and the gap between them.
//!    Read once a turn ([`ContestedLandFrame`]) — the fronts are consulted
//!    for every candidate of every site scan.
//! 2. **Posture — "a decent military".** The credit below is paid only while
//!    our `military_power` is at least [`CONTESTED_LAND_POWER_RATIO`] of the
//!    strongest front's rival AND at least [`CONTESTED_LAND_MIN_DEFENDERS`]
//!    land combat units stand. Outmatched, the gene is silent and the shipped
//!    terms keep the Settler home.
//! 3. **The credit.** A site is contested when it lies BETWEEN us and a
//!    front — nearer their city than our city is, and nearer our city than
//!    their city is — and within [`CONTESTED_LAND_RADIUS`] of their city,
//!    the ring their own Settlers reach next. It earns
//!    [`CONTESTED_LAND_BASE`] plus [`CONTESTED_LAND_PER_TILE`] for every
//!    tile of the gap it closes, capped at [`CONTESTED_LAND_CAP`]; the best
//!    front pays. The credit joins `settle_value_visible` and the global
//!    prefilter, so a contested site is never dropped before it is priced.
//! 4. **The provocation is waived where the ground is contested.** A
//!    credited site pays no `foreign_border_pressure`: pressing the border
//!    is the point. Everywhere else the penalty stands, and the six-a-tile
//!    city-distance term stands everywhere — it is what keeps the claim off
//!    the rival's doorstep, where loyalty would take it back.
//! 5. **The frontier holds.** An own city within
//!    [`CONTESTED_LAND_FRONTIER_RADIUS`] of a met major's city is a frontier
//!    city: its first Walls are worth [`CONTESTED_LAND_FRONTIER_WALLS`] more
//!    in `production_value`, and in peacetime it wants a garrison when no
//!    unit stands on it (`BasicAi::garrison_assignments_inner`, below any
//!    live pressure and nearest-to-the-neighbour first). Neither needs the
//!    posture: a frontier city outmatched needs its walls most.
//! 6. **When the contested land runs out**, no candidate carries a credit
//!    and the shipped ranking chooses the uncontested site — the fallback is
//!    the absence of the term, not a second rule.
//!
//! Counters: `contested_land:founded` (a city founded on a credited site).

use super::AdvancedAi;
use crate::ai::BasicAi;
use crate::game::Game;
use crate::Pos;

/// A met major with a city within this of one of ours is a neighbour — the
/// campaign planner's own reach (`city_campaign::CAMPAIGN_REACH`).
pub(crate) const CONTESTED_LAND_REACH: i32 = super::city_campaign::CAMPAIGN_REACH;
/// Ground within this of a neighbour's city is contested: the ring the
/// neighbour's next Settler reaches.
pub(crate) const CONTESTED_LAND_RADIUS: i32 = 8;
/// Site credit for any contested site.
pub(crate) const CONTESTED_LAND_BASE: f64 = 10.0;
/// Credit per tile of the gap to the neighbour the site closes.
pub(crate) const CONTESTED_LAND_PER_TILE: f64 = 3.0;
/// The most the credit may add to a site.
pub(crate) const CONTESTED_LAND_CAP: f64 = 28.0;
/// "Decent": our military power is at least this share of the strongest
/// neighbour's.
pub(crate) const CONTESTED_LAND_POWER_RATIO: f64 = 0.8;
/// ...and at least this many land combat units stand, however small the
/// neighbour is.
pub(crate) const CONTESTED_LAND_MIN_DEFENDERS: usize = 2;
/// An own city within this of a met major's city is a frontier city.
pub(crate) const CONTESTED_LAND_FRONTIER_RADIUS: i32 = 8;
/// What a frontier city's first Walls are worth on top of the ordinary
/// building value; the threatened-city Walls bonus is 320.
pub(crate) const CONTESTED_LAND_FRONTIER_WALLS: f64 = 240.0;

/// One front: a neighbour's city, our city nearest it, and the gap between.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ContestedFront {
    pub(crate) rival: usize,
    pub(crate) their_city: Pos,
    pub(crate) our_city: Pos,
    pub(crate) gap: i32,
}

/// The fronts and the posture as they stood when first asked this turn.
/// Keyed on the turn, the seat, and the city and unit counts, so a founding
/// or a casualty re-reads them and a site scan of five hundred candidates
/// reads them once.
#[derive(Clone, Debug, Default)]
pub(crate) struct ContestedLandFrame {
    turn: Option<u32>,
    pid: usize,
    cities: usize,
    units: usize,
    pub(crate) fronts: Vec<ContestedFront>,
    pub(crate) posture: bool,
}

impl ContestedLandFrame {
    fn matches(&self, g: &Game, pid: usize) -> bool {
        self.turn == Some(g.turn)
            && self.pid == pid
            && self.cities == g.cities.len()
            && self.units == g.units.len()
    }

    fn build(g: &Game, pid: usize) -> Self {
        let fronts = Self::fronts(g, pid);
        let posture = Self::posture(g, pid, &fronts);
        Self {
            turn: Some(g.turn),
            pid,
            cities: g.cities.len(),
            units: g.units.len(),
            fronts,
            posture,
        }
    }

    /// Every neighbour's city within [`CONTESTED_LAND_REACH`] of one of
    /// ours, with our nearest city and the gap. Deterministic: cities are
    /// visited in id order.
    fn fronts(g: &Game, pid: usize) -> Vec<ContestedFront> {
        let ours: Vec<Pos> = g
            .player_city_ids(pid)
            .into_iter()
            .filter_map(|cid| g.cities.get(&cid).map(|city| city.pos))
            .collect();
        if ours.is_empty() {
            return Vec::new();
        }
        g.cities
            .values()
            .filter(|city| city.owner != pid)
            .filter(|city| {
                let owner = &g.players[city.owner];
                owner.alive
                    && !owner.is_minor
                    && !owner.is_barbarian
                    && g.has_met(pid, city.owner)
                    && !g.same_team(pid, city.owner)
                    && !g.is_at_war(pid, city.owner)
            })
            .filter_map(|city| {
                let (gap, our_city) = ours
                    .iter()
                    .map(|our| (g.wdist(*our, city.pos), *our))
                    .min()?;
                (gap <= CONTESTED_LAND_REACH).then_some(ContestedFront {
                    rival: city.owner,
                    their_city: city.pos,
                    our_city,
                    gap,
                })
            })
            .collect()
    }

    /// "A decent military": see the module doc, item 2.
    fn posture(g: &Game, pid: usize, fronts: &[ContestedFront]) -> bool {
        if fronts.is_empty() {
            return false;
        }
        let defenders = g
            .units
            .values()
            .filter(|unit| unit.owner == pid)
            .filter(|unit| g.rules.units[unit.kind].class == "military")
            .filter(|unit| {
                matches!(
                    g.rules.units[unit.kind].domain.as_deref(),
                    None | Some("land")
                )
            })
            .count();
        if defenders < CONTESTED_LAND_MIN_DEFENDERS {
            return false;
        }
        let strongest = fronts
            .iter()
            .map(|front| g.military_power(front.rival))
            .fold(0.0_f64, f64::max);
        g.military_power(pid) >= CONTESTED_LAND_POWER_RATIO * strongest
    }
}

impl AdvancedAi {
    /// The gene's flag, read from the base controller that owns the
    /// garrison. See `BasicAi::contested_land_first`.
    pub fn contested_land_first(&self) -> bool {
        self.base.contested_land_first
    }

    /// This turn's fronts and posture, built on first use. See
    /// [`ContestedLandFrame`].
    pub(crate) fn contested_land_frame(
        &self,
        g: &Game,
        pid: usize,
    ) -> std::cell::Ref<'_, ContestedLandFrame> {
        if !self.contested_land_frame.borrow().matches(g, pid) {
            let rebuilt = ContestedLandFrame::build(g, pid);
            *self.contested_land_frame.borrow_mut() = rebuilt;
        }
        self.contested_land_frame.borrow()
    }

    /// The front that makes `pos` contested, and what the site earns for it:
    /// the module doc's item 3. `None` with the gene off, without the
    /// posture, or for a site that is not between us and a neighbour.
    pub(crate) fn contested_land_front(
        &self,
        g: &Game,
        pid: usize,
        pos: Pos,
    ) -> Option<(ContestedFront, f64)> {
        if !self.base.contested_land_first || self.base.minor || self.base.barb {
            return None;
        }
        let frame = self.contested_land_frame(g, pid);
        if !frame.posture {
            return None;
        }
        frame
            .fronts
            .iter()
            .filter_map(|front| {
                let to_rival = g.wdist(pos, front.their_city);
                let to_us = g.wdist(pos, front.our_city);
                if to_rival > CONTESTED_LAND_RADIUS || to_rival >= front.gap || to_us >= front.gap {
                    return None;
                }
                let closed = f64::from(front.gap - to_rival);
                let credit = (CONTESTED_LAND_BASE + closed * CONTESTED_LAND_PER_TILE)
                    .min(CONTESTED_LAND_CAP);
                Some((*front, credit))
            })
            .max_by(|a, b| {
                a.1.total_cmp(&b.1)
                    .then(b.0.their_city.cmp(&a.0.their_city))
            })
    }

    /// The credit alone. Zero wherever `contested_land_front` is `None`.
    pub(crate) fn contested_land_credit(&self, g: &Game, pid: usize, pos: Pos) -> f64 {
        self.contested_land_front(g, pid, pos)
            .map_or(0.0, |(_, credit)| credit)
    }

    /// Whether `foreign_border_pressure` is waived at `pos`: the module
    /// doc's item 4 — only where the ground is contested.
    pub(crate) fn contested_land_waives_provocation(&self, g: &Game, pid: usize, pos: Pos) -> bool {
        self.contested_land_front(g, pid, pos).is_some()
    }

    /// What Walls are worth to a frontier city on top of their ordinary
    /// value: the module doc's item 5. Zero with the gene off, for any
    /// building but the first Walls, or for a city that is not on a
    /// frontier.
    pub(crate) fn contested_land_walls_value(
        &self,
        g: &Game,
        pid: usize,
        city: Pos,
        building: &str,
    ) -> f64 {
        if building != "walls" || !self.base.contested_frontier_city(g, pid, city) {
            return 0.0;
        }
        CONTESTED_LAND_FRONTIER_WALLS
    }
}

impl BasicAi {
    /// The geometry of `contested_frontier_city`, without the flag: the
    /// distance from `city` to the nearest city of a met, living major that
    /// is not a team-mate, when that is within
    /// [`CONTESTED_LAND_FRONTIER_RADIUS`].
    pub(crate) fn contested_frontier_distance_inner(
        g: &Game,
        pid: usize,
        city: Pos,
    ) -> Option<i32> {
        g.cities
            .values()
            .filter(|other| other.owner != pid)
            .filter(|other| {
                let owner = &g.players[other.owner];
                owner.alive
                    && !owner.is_minor
                    && !owner.is_barbarian
                    && g.has_met(pid, other.owner)
                    && !g.same_team(pid, other.owner)
            })
            .map(|other| g.wdist(other.pos, city))
            .min()
            .filter(|distance| *distance <= CONTESTED_LAND_FRONTIER_RADIUS)
    }

    /// `contested_land_first`: how far `city` stands from the nearest
    /// neighbour's city when it is a frontier city, `None` otherwise or with
    /// the gene off. Never for a minor or the barbarian seat.
    pub(crate) fn contested_frontier_distance(
        &self,
        g: &Game,
        pid: usize,
        city: Pos,
    ) -> Option<i32> {
        if !self.contested_land_first || self.minor || self.barb {
            return None;
        }
        Self::contested_frontier_distance_inner(g, pid, city)
    }

    /// Whether `city` is a frontier city under `contested_land_first`.
    pub(crate) fn contested_frontier_city(&self, g: &Game, pid: usize, city: Pos) -> bool {
        self.contested_frontier_distance(g, pid, city).is_some()
    }
}
