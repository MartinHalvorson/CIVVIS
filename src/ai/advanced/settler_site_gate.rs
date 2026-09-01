//! `settler-site-gate`: a city starts a Settler only while the map still holds
//! an acceptable site for it — worth founding, outside every visible rival's
//! Loyalty sphere, and not already the target of a Settler in flight.
//!
//! ## The live evidence (Emperor, run `civvis-20260901T182050Z`, Rome, Continents/Small)
//!
//! Cities stalled at 5–6 from t45 to t105 while two to four Settlers stood
//! alive the whole time. Settler 1703949 took 37 `MOVE_TO` orders to 31
//! distinct targets between t60 and t105 and never founded; two more were
//! lost on the road (t97, t103). Its targets were sites worth 13.8, 18.9 and
//! 24.1 — founded sites on the same map were worth 96–140 — and every site
//! nearer home was retired on approach as a rival city came into view
//! (*"within five tiles of a visible rival city and would lose 19.0 Loyalty a
//! turn"*, *"Stranded Settler sets aside 5 site(s) inside a rival's sphere"*).
//! Meanwhile the production arm kept paying `920 + 4 × site` for a Settler
//! whenever ANY site existed within eleven tiles: at t80 and t90 three cities
//! were building Settlers at once, and at t100 Antium paused a project for
//! another (*"worth 64; the peaceful science plan has 6 cities and wants
//! 15"*). Each was ~100 production spent on a unit with nowhere to go.
//!
//! ## What the gate does
//!
//! It asks the walker's questions before the Settler exists. The city's site
//! list is taken best-first; sites the walker would refuse are dropped
//! (`exhaustion_site_unpriceable`, `inside_rival_sphere`); then seats are
//! counted the way `map_settlement_room` counts them — a seat must stand
//! [`SETTLEMENT_SPACING`] from every target a live Settler already holds and
//! from every seat counted before it. A new Settler is priced only when the
//! seats outnumber the Settlers that still need one and the best seat is
//! worth at least [`SETTLER_SITE_GATE_FLOOR`]; otherwise the arm returns its
//! ordinary veto and journals why, once per city per review.
//!
//! Off (the default) nothing here runs; the arm prices exactly as before.
use super::{AdvancedAi, SETTLEMENT_SPACING};
use crate::game::Game;
use crate::Pos;

/// The least a site may be worth before a city will build a Settler for it.
///
/// On the run above the sites Settlers were sent to and never founded were
/// worth 13.8, 18.9 and 24.1; the seven that were founded were worth 96–140.
/// The arm's own price adds `4 × worth`, so this floor is the point where
/// the site contributes less than a fifth of the Settler's base value.
pub(super) const SETTLER_SITE_GATE_FLOOR: f64 = 40.0;

/// Why a city was not allowed to start a Settler.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) enum SettlerSiteGateHold {
    /// No site the walker would accept lies within the search radius.
    NoSite,
    /// Every acceptable seat is spoken for by a Settler already alive.
    AllClaimed { seats: usize, unseated: usize },
    /// The best free seat is not worth a Settler.
    BelowFloor { worth: f64 },
}

impl SettlerSiteGateHold {
    pub(super) fn describe(&self) -> String {
        match self {
            Self::NoSite => "no site the walker would accept lies within reach".to_string(),
            Self::AllClaimed { seats, unseated } => format!(
                "{seats} free seat(s) for {unseated} settler(s) still without one; a new \
                 settler would have nowhere to go"
            ),
            Self::BelowFloor { worth } => format!(
                "the best free seat is worth {worth:.1}, under the {SETTLER_SITE_GATE_FLOOR:.0} \
                 floor; founded sites on this rung are worth 96–140"
            ),
        }
    }
}

impl AdvancedAi {
    /// The pure verdict: `sites` best-first as `settle_sites` returns them,
    /// `claimed` the targets live Settlers already hold, `settlers` how many
    /// Settlers are alive (claimed ones included).
    pub(super) fn settler_site_gate_verdict(
        g: &Game,
        sites: &[(Pos, f64)],
        claimed: &[Pos],
        settlers: usize,
    ) -> Result<(Pos, f64), SettlerSiteGateHold> {
        if sites.is_empty() {
            return Err(SettlerSiteGateHold::NoSite);
        }
        let mut seats: Vec<(Pos, f64)> = Vec::new();
        for &(pos, worth) in sites {
            let free = claimed
                .iter()
                .chain(seats.iter().map(|(seat, _)| seat))
                .all(|taken| g.wdist(*taken, pos) >= SETTLEMENT_SPACING);
            if free {
                seats.push((pos, worth));
            }
        }
        // A Settler that already holds a target has its seat; the rest queue
        // for the free ones ahead of any Settler this city would start now.
        let unseated = settlers.saturating_sub(claimed.len());
        if seats.len() <= unseated {
            return Err(SettlerSiteGateHold::AllClaimed {
                seats: seats.len(),
                unseated,
            });
        }
        let best = seats[unseated];
        if best.1 < SETTLER_SITE_GATE_FLOOR {
            return Err(SettlerSiteGateHold::BelowFloor { worth: best.1 });
        }
        Ok(best)
    }

    /// The gate for one city: its acceptable sites within the ordinary radius
    /// (the whole map once Shipbuilding is known), the targets its live
    /// Settlers hold, and the verdict.
    pub(super) fn settler_site_gate(
        &self,
        g: &Game,
        pid: usize,
        from: Pos,
        settlers: usize,
    ) -> Result<(Pos, f64), SettlerSiteGateHold> {
        let mut sites = self.settle_sites(g, pid, from, 11);
        if sites.is_empty() && g.players[pid].techs.contains(&crate::name!("shipbuilding")) {
            sites = self.settle_sites(g, pid, from, g.map.width + g.map.height);
        }
        sites.retain(|(site, _)| {
            self.exhaustion_site_unpriceable(g, *site).is_none()
                && !Self::inside_rival_sphere(g, pid, *site)
        });
        let claimed: Vec<Pos> = self
            .settler_targets
            .iter()
            .filter(|(uid, _)| {
                g.units
                    .get(uid)
                    .is_some_and(|unit| unit.owner == pid && unit.kind == "settler")
            })
            .map(|(_, target)| *target)
            .collect();
        Self::settler_site_gate_verdict(g, &sites, &claimed, settlers)
    }
}
