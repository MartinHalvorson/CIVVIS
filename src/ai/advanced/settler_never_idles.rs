//! `settler-never-idles`: a Settler always has somewhere to go, and a hold
//! is a bounded, named exception rather than the default.
//!
//! Operator, 2026-08-27: *"our settlers often just sit around in the capital
//! or other cities. this is a huge mistake. investigate how this was allowed
//! to happen and fix the behavior so this can't happen."*
//!
//! ## How it was allowed to happen
//!
//! `advanced_settler_step` grew, one live anecdote at a time, more than a
//! dozen branches that leave a Settler exactly where it stands: a Loyalty
//! forecast that refuses a site, a fog guess (`beyond_loyalty_reach`) that
//! refuses every frontier site while any plot within nine tiles is
//! unexplored, a safe-step guard that refuses every neighbour, a guard that
//! has not arrived, a raider's reach it will not enter. Each hold was right
//! for the run that motivated it, and none was ever charged for the turns
//! it costs, because every one of the holds that matter is gated on a
//! host-only gene (`frontier-loyalty`, `live-formationless-settler-shadow`)
//! or on `loyalty-rate-alarm`, and the screen cannot price host-only genes
//! (`docs/AI_GAPS.md`, "the settler-idling lane cannot be screened at all").
//! Worst of all, when every preferred site is refused the code *held*
//! (`return false`) instead of asking a wider question — and every refused
//! site was retired for thirty standard turns, so the refusals compounded
//! into a permanent "no target".
//!
//! `settler_idle_census` (`src/ai/advanced/settler_idle_census.rs`) put
//! numbers on it, 6-player 60×38 Online, four maps each:
//!
//! | genome on a native board | settler-turns idle | idle on an own city tile | settlers idle ≥10 turns in a city | never moved |
//! |---|---|---|---|---|
//! | deployment (`enable_engine_repairs`) | 24.9% | 6.0% of all settler-turns | 4.0% | 11 of 151 |
//! | live seat (`enable_live_bridge`) | **87.0%** | **32.7%** of all settler-turns | **27.5%** | 76 of 389 |
//!
//! On the live seat's genome 62.5% of the idle turns were "no target" —
//! the search returned nothing and the code held; the five worst Settlers
//! stood in the city that built them for 149–185 turns and never moved.
//!
//! ## What the gene does
//!
//! 1. **Exhaustion never holds.** When the preferred search (every filter,
//!    the Loyalty verdict included) returns nothing, the Settler asks two
//!    wider questions before it stands still: the advanced ranking with the
//!    same retired-site, threat-deferral and peer-reservation guards, refusing
//!    only a *concrete* revolt inside [`STRANDED_SITE_MIN_HOLD_TURNS`] (twenty)
//!    (`settler_exhaustion_target`, tier 2). An explicit Science lane keeps the
//!    full growth-horizon Loyalty guard instead of accepting that compromise.
//!    Then any legal reachable site at all, nearest first (tier 3). Failing
//!    both it asks the explored frontier for a safe reveal step after its idle
//!    patience. It founds where it stands only if the engine allows, and
//!    otherwise says so in the journal — a Settler that holds is never silent.
//! 2. **A watchdog bounds every other hold.** A Settler that has stood on
//!    the same tile for [`SETTLER_IDLE_PATIENCE`] turns stops trusting the
//!    branch that held it and marches on one rule only: never end the turn
//!    on a tile a visible hostile can reach next turn (`civilian_safe_at`
//!    over the barbarian reach, plus the hex-distance reach of every visible
//!    at-war major unit). That is the rule `civilian-out-of-reach` already
//!    plays and the screen priced at +29/+20/+31 wins per 10,000 seats; a
//!    softer risk score, a guard that has not come, a forecast about fogged
//!    ground — none of those may hold a Settler past its patience.
//! 3. **The guard wait returns the turn.** Under the gene the live seat's
//!    "waits for its guard" no longer reports the turn as spent, so the
//!    watchdog can see it.
//!
//! Off, every touched path is byte-identical to before.

use super::civilian_safety::{BarbarianReach, REACH_SCAN_RADIUS};
use super::{
    AdvancedAi, VictoryTarget, EARLY_SETTLER_HOME_RADIUS, SETTLEMENT_GLOBAL_PREFILTER_LIMIT,
    SETTLER_DEAD_SITE_AVOID_TURNS, SETTLER_STEP_RISK_LIMIT,
};
use crate::ai::BasicAi;
use crate::game::{Action, Game};
use crate::think;
use crate::Pos;
use std::collections::HashSet;

/// Turns a Settler may stand on one tile before the watchdog marches it on
/// the exact-reach rule alone. Two: one turn is the ordinary weather of a
/// guard arriving or a raider passing; the census's idle-in-city streaks
/// run to 185.
pub(super) const SETTLER_IDLE_PATIENCE: u32 = 2;
/// A site the engine's own Loyalty calculation says would revolt inside
/// this many turns is doomed however stranded the Settler is; anything
/// slower is a city for a while, which beats a Settler for ever. Half the
/// forty-turn growth horizon the preferred verdict uses
/// (`SETTLE_TARGET_LOYALTY_RISK_TURNS`): a city that holds twenty turns has
/// grown and built before it is lost, and it is only ever chosen when no
/// site passes the preferred verdict at all.
pub(super) const STRANDED_SITE_MIN_HOLD_TURNS: f64 = 20.0;
/// How far a stranded Settler looks for any legal site before Shipbuilding.
const STRANDED_SITE_RADIUS: i32 = 14;
/// The bounded reveal radius used when no legal city target survives. Reusing
/// the exhaustion radius keeps the recovery cheap and ensures it can expose
/// the same nearby ground the next search will price.
const STRANDED_FRONTIER_RADIUS: i32 = STRANDED_SITE_RADIUS;
/// How many forecast refusals the exhaustion search pays before it stops
/// asking the forecast; each is a speculative founding on a cloned board.
const STRANDED_FORECAST_RETRIES: usize = 4;
/// Turns a Settler found stranded on a tile waits before the exhaustion
/// search is asked again from that tile. The board that stranded it changes
/// slowly — a razed rival city, a border that recedes — and the search is
/// the dearest thing a Settler does; a Settler that moves is asked at once.
pub(super) const STRANDED_RECHECK_TURNS: u32 = 5;

impl AdvancedAi {
    /// Advance this Settler's idle streak, once per turn, and return it.
    /// Zero the turn it appears and every turn it stands somewhere new.
    pub(super) fn note_settler_idle(&mut self, g: &Game, uid: u32) -> u32 {
        let pos = g.units[&uid].pos;
        let turn = g.turn;
        self.settler_idle_streak
            .retain(|other, _| g.units.contains_key(other));
        let entry = self
            .settler_idle_streak
            .entry(uid)
            .or_insert((pos, turn, 0));
        if entry.1 != turn {
            let streak = if entry.0 == pos { entry.2 + 1 } else { 0 };
            *entry = (pos, turn, streak);
        }
        entry.2
    }

    /// How long this Settler has stood on its tile, as last noted.
    pub(super) fn settler_idle_streak(&self, uid: u32) -> u32 {
        self.settler_idle_streak
            .get(&uid)
            .map_or(0, |(_, _, streak)| *streak)
    }

    /// Whether this Settler was found stranded on its current tile inside
    /// the last [`STRANDED_RECHECK_TURNS`] turns, so the exhaustion search
    /// need not be paid again yet.
    pub(super) fn settler_stranded_recently(&self, g: &Game, uid: u32) -> bool {
        self.settler_stranded_at
            .get(&uid)
            .is_some_and(|(pos, turn)| {
                g.units.get(&uid).is_some_and(|unit| unit.pos == *pos)
                    && g.turn < turn.saturating_add(STRANDED_RECHECK_TURNS)
            })
    }

    /// The arrival verdict for a site the exhaustion search chose: broad fog
    /// guesses had their say when the walk began, but a newly revealed major
    /// border with no visible city is concrete evidence the forecast cannot
    /// price. Otherwise use the same [`STRANDED_SITE_MIN_HOLD_TURNS`] floor
    /// the choice used.
    /// Without this a relaxed target was refused on arrival by the strict
    /// verdict, retired, chosen again by the next exhaustion search, walked
    /// to again — 1,062 idle turns of that loop in eight live-genome games.
    pub(super) fn relaxed_arrival_verdict(
        &self,
        g: &Game,
        pid: usize,
        site: Pos,
    ) -> Option<String> {
        if let Some(why) = self.exhaustion_site_unpriceable(g, pid, site) {
            return Some(why.to_string());
        }
        Self::settle_site_forecast_revolt(g, pid, site)
            .filter(|(_, turns)| *turns < STRANDED_SITE_MIN_HOLD_TURNS)
            .map(|(per_turn, turns)| {
                format!(
                    "the city would lose {:.1} Loyalty a turn and revolt in about {:.0} turns",
                    -per_turn,
                    turns.ceil()
                )
            })
    }

    /// An explicit Science lane cannot spend a Settler on a city that is
    /// already forecast to revolt. Other lanes retain the never-idles
    /// contract's bounded twenty-turn exception: a colony that holds for a
    /// while can still be better than a Settler standing still forever.
    fn science_targeted(&self, g: &Game) -> bool {
        self.active_victory_target(g) == Some(VictoryTarget::Science)
    }

    /// Apply the exhaustion lane's Loyalty floor, with the stricter Science
    /// contract. `settle_site_forecast_revolt` already limits the result to
    /// the ordinary forty-turn growth horizon, so Science refuses every
    /// forecasted revolt while other lanes only refuse the urgent half.
    fn exhaustion_site_revolt(&self, g: &Game, pid: usize, site: Pos) -> Option<(f64, f64)> {
        Self::settle_site_forecast_revolt(g, pid, site)
            .filter(|(_, turns)| self.science_targeted(g) || *turns < STRANDED_SITE_MIN_HOLD_TURNS)
    }

    /// Why the exhaustion search may not take `site` under `frontier-loyalty`.
    /// A met major's border with no visible city is host evidence of pressure
    /// the mirrored forecast cannot price. Keep this narrow host-only floor
    /// even when the broader, screen-negative `exhaustion-loyalty-guard` is off.
    /// Native boards have no `unseen_major_borders`, so their search is unchanged.
    ///
    /// `hidden-border-yields-to-our-pressure`: a site our own cities already
    /// press yields here too, as it does in `settle_site_frontier_loyalty_verdict`
    /// (live Immortal civvis-20261030T171157Z logged sixteen refusals on this
    /// path and none on the verdict's). See `advanced/hidden_border_reach.rs`.
    pub(super) fn exhaustion_site_unpriceable(
        &self,
        g: &Game,
        pid: usize,
        site: Pos,
    ) -> Option<&'static str> {
        (self.frontier_loyalty
            && Self::beside_unresolved_major_border(g, site)
            && !self.hidden_border_yields(g, pid, site))
        .then_some("a rival border within five tiles may hide the city that would press it")
    }

    /// Whether `site` sits inside a rival major's Loyalty sphere: a visible
    /// rival-major city stands within the nine-tile pressure radius and no
    /// own city stands as near. A city founded there loses about twenty
    /// Loyalty a turn from turn one — run `civvis-20260829T050508Z` t56–t69
    /// forecast −22 on each of the five plots nearest a stranded Settler
    /// that had wandered between two Spanish cities, spent the forecast
    /// retries on them, and held fourteen turns while sites eight tiles
    /// nearer home would have passed. The forecast is the judge; this is the
    /// cheap sieve that keeps its retries for plots that can pass.
    pub(super) fn inside_rival_sphere(g: &Game, pid: usize, site: Pos) -> bool {
        let mut nearest_rival: Option<i32> = None;
        let mut nearest_own: Option<i32> = None;
        for city in g.cities.values() {
            let distance = g.wdist(city.pos, site);
            if city.owner == pid {
                nearest_own = Some(nearest_own.map_or(distance, |d| d.min(distance)));
            } else if !g.players[city.owner].is_minor && !g.players[city.owner].is_barbarian {
                nearest_rival = Some(nearest_rival.map_or(distance, |d| d.min(distance)));
            }
        }
        match (nearest_rival, nearest_own) {
            (Some(rival), Some(own)) => rival <= 9 && rival <= own,
            (Some(rival), None) => rival <= 9,
            _ => false,
        }
    }

    /// Drop sites beside an unresolved major border when frontier protection
    /// is active. The broader rival-sphere sieve still needs the optional
    /// exhaustion gene or an explicit Science contract.
    fn set_aside_unpriceable_sites(&self, g: &Game, pid: usize, candidates: &mut Vec<(Pos, f64)>) {
        let before = candidates.len();
        candidates.retain(|(pos, _)| self.exhaustion_site_unpriceable(g, pid, *pos).is_none());
        let unpriceable = before - candidates.len();
        if unpriceable > 0 {
            think!(self.journal(), Expansion, Detail,
                   "Stranded Settler sets aside {unpriceable} unpriceable site(s)";
                   "each lies within five tiles of a rival border whose city may be hidden; \
                    the forecast cannot price that Loyalty pressure, and the exhaustion \
                    search will not guess at a city it would hand to that rival");
        }
        if !self.exhaustion_loyalty_guard && !self.science_targeted(g) {
            return;
        }
        let before = candidates.len();
        candidates.retain(|(pos, _)| !Self::inside_rival_sphere(g, pid, *pos));
        let in_sphere = before - candidates.len();
        if in_sphere > 0 {
            think!(self.journal(), Expansion, Detail,
                   "Stranded Settler sets aside {in_sphere} site(s) inside a rival's sphere";
                   "each stands nearer a visible rival major's city than any of ours, within \
                    its nine-tile Loyalty reach; the forecast's retries are kept for plots \
                    that can pass");
        }
    }

    /// The wider questions asked when the preferred search returns nothing.
    ///
    /// Tier 2: the advanced ranking over a stranded radius, with this
    /// Settler's retired sites, hysteresis avoidance, the empire's threat
    /// deferrals and the fog guesses set aside; a candidate is refused only
    /// when the engine's own Loyalty calculation of a city founded there
    /// revolts inside [`STRANDED_SITE_MIN_HOLD_TURNS`]. An explicit Science
    /// lane refuses every forecasted revolt in the normal growth horizon.
    /// Tier 3: any legal reachable site, nearest first, with the same
    /// per-Settler exclusions.
    /// `None` when the map holds nothing this Settler can reach.
    pub(super) fn settler_exhaustion_target(&self, g: &Game, pid: usize, uid: u32) -> Option<Pos> {
        let from = g.units[&uid].pos;
        // The normal target picker receives this exception explicitly, but
        // the exhaustion fallback is also reached after a target was dropped
        // for an unsafe route step. Keep the same cooldown here or the
        // never-idles lane immediately resurrects the very site hysteresis
        // retired (the live t103-t111 `(13,31)` loop). Expiry is checked here
        // as well so direct watchdog/test callers do not inherit stale state.
        let avoided = self
            .settler_avoid
            .get(&uid)
            .and_then(|(position, until)| (*until > g.turn).then_some(*position));
        let radius = if g.players[pid].techs.contains(&crate::name!("shipbuilding")) {
            g.map.width + g.map.height
        } else {
            STRANDED_SITE_RADIUS
        };
        let mut ranked: Vec<(Pos, f64)> = self
            .settle_sites_with_limit(
                g,
                pid,
                from,
                radius,
                Some(SETTLEMENT_GLOBAL_PREFILTER_LIMIT),
            )
            .into_iter()
            .filter(|(pos, value)| {
                !g.blocked_city_sites.contains(pos)
                    && self.early_settler_site_allowed(g, pid, uid, *pos)
                    && !self.settler_site_is_dead(uid, *pos)
                    && Some(*pos) != avoided
                    && (!self.settler_threat_detour_on()
                        || !self.settler_threat_deferrals.contains_key(pos))
                    && !self.settler_target_reserved_by_other(g, pid, uid, *pos)
                    // `settler-target-floor`: exhaustion asks wider, not worse.
                    && self.settler_target_clears_floor(g, from, *pos, *value)
            })
            .collect();
        // Exhausting the preferred filters does not make travel free. Keep
        // the same turn price as the normal target search, including the
        // growing cost of leaving an older Settler without a city.
        let turn_price = self.settle_sooner_walk_price(g, uid);
        if let Some((_, moves)) = turn_price {
            let costs = Self::settle_sooner_walk_costs(g, uid, radius);
            for (position, value) in &mut ranked {
                let movement_cost = costs
                    .get(position)
                    .copied()
                    .unwrap_or_else(|| g.wdist(from, *position) as f64 * moves.max(1.0) / 2.0);
                *value -= Self::settle_sooner_walk_cost(turn_price, movement_cost);
            }
            ranked.sort_by(|a, b| b.1.total_cmp(&a.1).then(a.0.cmp(&b.0)));
        }
        self.set_aside_unpriceable_sites(g, pid, &mut ranked);
        for _ in 0..=STRANDED_FORECAST_RETRIES {
            let Some((site, value)) = BasicAi::first_reachable_settle_site(g, uid, &ranked) else {
                break;
            };
            match self.exhaustion_site_revolt(g, pid, site) {
                Some((per_turn, turns)) => {
                    think!(self.journal(), Expansion, Detail,
                           "Settler skips a doomed site at {site:?}";
                           "stranded, it would still take a site the forecast set aside — but this \
                            one loses {:.1} Loyalty a turn and revolts in about {turns:.0}", -per_turn;
                           site);
                    ranked.retain(|(pos, _)| *pos != site);
                }
                _ => {
                    think!(self.journal(), Expansion, Detail,
                           "Settler takes a site the preferred search refused";
                           "nothing passed every filter, so {site:?} (worth {value:.1}) is taken \
                            rather than standing still"; site);
                    return Some(site);
                }
            }
        }
        // Tier 3 normally asks for the nearest legal site.  The live bridge
        // has one additional failure mode: the nearest legal fallback can be
        // a visibly exposed, low-value plot even while a safer and more
        // defensible site is available.  That is exactly how a stranded
        // Settler was sent into the Gaul lane on t110 of the managed run.
        // Reuse the normal site value and route-risk model for the live-only
        // fallback, while keeping the historical nearest-site behavior for
        // native/evaluator controllers.
        let live_fallback_safety = self.live_settler_capture_lessons && self.settlement_safety;
        let science_targeted = self.science_targeted(g);
        let live_fallback_loyalty = science_targeted
            || (live_fallback_safety && (self.base.loyalty_rate_alarm || self.frontier_loyalty));
        let visible = live_fallback_safety.then(|| self.battlefront_visibility(g, pid));
        let mut legal: Vec<(Pos, f64)> = g
            .wdisk(from, radius)
            .into_iter()
            .filter(|pos| {
                self.base.valid_settle_site(g, pid, *pos)
                    && !g.blocked_city_sites.contains(pos)
                    && self.early_settler_site_allowed(g, pid, uid, *pos)
                    && !self.settler_site_is_dead(uid, *pos)
                    && Some(*pos) != avoided
                    && (!self.settler_threat_detour_on()
                        || !self.settler_threat_deferrals.contains_key(pos))
                    && !self.settler_target_reserved_by_other(g, pid, uid, *pos)
            })
            .filter_map(|pos| {
                if !live_fallback_safety {
                    if science_targeted && self.settle_site_loyalty_verdict(g, pid, pos).is_some() {
                        return None;
                    }
                    return Some((pos, -(g.wdist(from, pos) as f64)));
                }
                let visible = visible
                    .as_ref()
                    .expect("the live fallback owns a visibility frame");
                // A fallback target is not allowed to be a tile a visible
                // hostile can already take next turn.  If every legal plot
                // is exposed, returning no target lets the emergency flee
                // pass/stranded watchdog hold or retreat instead of knowingly
                // marching into the capture envelope.
                let tile_risk = self.settlement_tile_risk(g, pid, Some(uid), pos, visible);
                if tile_risk > SETTLER_STEP_RISK_LIMIT {
                    return None;
                }
                // The ranked exhaustion tier already forecasts Loyalty.  Keep
                // that same guard on the live nearest/legal fallback: otherwise
                // exhausting the ranked list would make the bridge deliberately
                // choose a site the forecast just proved would revolt.
                if live_fallback_loyalty && self.settle_site_loyalty_verdict(g, pid, pos).is_some()
                {
                    return None;
                }
                let site_value = self.settle_value_visible(g, pid, pos, visible);
                let route_penalty = g
                    .path_to(uid, pos)
                    .map(|path| {
                        let (movement_cost, route_risk) =
                            self.settlement_route_risk(g, pid, uid, &path, visible);
                        movement_cost * 0.8 + route_risk
                    })
                    .unwrap_or(0.0);
                let distance_penalty = if radius > 12 { 0.78 } else { 1.25 };
                Some((
                    pos,
                    site_value - distance_penalty * g.wdist(from, pos) as f64 - route_penalty,
                ))
            })
            .collect();
        legal.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap().then(a.0.cmp(&b.0)));
        self.set_aside_unpriceable_sites(g, pid, &mut legal);
        if self.exhaustion_loyalty_guard || science_targeted {
            // This tier ran no forecast at all: of the sixteen nearest-legal
            // foundings in the King runs of 2026-08-27..29, six read a
            // negative Loyalty rate at their first reading and three
            // revolted. Under the gene it pays the same concrete-revolt check
            // as the ranked tier, retries included; a Settler that then holds
            // is named by `settler_stranded`.
            for _ in 0..=STRANDED_FORECAST_RETRIES {
                let (site, _) = BasicAi::first_reachable_settle_site(g, uid, &legal)?;
                match self.exhaustion_site_revolt(g, pid, site) {
                    Some((per_turn, turns)) => {
                        think!(self.journal(), Expansion, Detail,
                               "Settler skips a doomed nearest site at {site:?}";
                               "no ranked site is reachable and the nearest legal one would be \
                                taken — but this one loses {:.1} Loyalty a turn and revolts in \
                                about {turns:.0}", -per_turn;
                               site);
                        legal.retain(|(pos, _)| *pos != site);
                    }
                    _ => {
                        if live_fallback_safety {
                            think!(self.journal(), Expansion, Detail,
                                   "Settler takes the safest reachable legal site";
                                   "no ranked site is reachable; {site:?} is {} tiles away and \
                                    the forecast says it holds",
                                   g.wdist(from, site); site);
                        } else {
                            think!(self.journal(), Expansion, Detail,
                                   "Settler takes the nearest legal site at {site:?}";
                                   "no ranked site is reachable; a city {} tiles away beats a \
                                    Settler standing still, and the forecast says it holds",
                                   g.wdist(from, site); site);
                        }
                        return Some(site);
                    }
                }
            }
            return None;
        }
        let site = BasicAi::first_reachable_settle_site(g, uid, &legal).map(|(pos, _)| pos)?;
        if live_fallback_safety {
            think!(self.journal(), Expansion, Detail,
                   "Settler takes the safest reachable legal site";
                   "no ranked site is reachable; {site:?} is {} tiles away and beats a Settler \
                    standing still",
                   g.wdist(from, site); site);
        } else {
            think!(self.journal(), Expansion, Detail,
                   "Settler takes the nearest legal site at {site:?}";
                   "no ranked site is reachable; a city {} tiles away beats a Settler standing \
                    still",
                   g.wdist(from, site); site);
        }
        Some(site)
    }

    /// Nothing is reachable: found here if the engine allows and the city
    /// would hold, else say so. Never silent.
    pub(super) fn settler_stranded(&mut self, g: &mut Game, pid: usize, uid: u32) -> bool {
        let here = g.units[&uid].pos;
        if let Some(home) = self.early_settler_homeward_target(g, uid) {
            return self.return_early_settler_home(g, pid, uid, home);
        }
        let science_targeted = self.science_targeted(g);
        self.settler_targets.remove(&uid);
        self.settler_stalls.remove(&uid);
        self.settler_closest.remove(&uid);
        if self.early_settler_site_allowed(g, pid, uid, here)
            && g.can_found_city(uid)
            && self.exhaustion_site_unpriceable(g, pid, here).is_none()
            && !((self.exhaustion_loyalty_guard || science_targeted)
                && Self::inside_rival_sphere(g, pid, here))
            && self.exhaustion_site_revolt(g, pid, here).is_none()
        {
            let worth = self.settle_value(g, pid, here);
            let founded = g.apply(pid, &Action::FoundCity { unit: uid }).is_ok();
            if founded {
                think!(self.journal(), Expansion, Decision,
                       "Founding where the stranded settler stands at {here:?}";
                       "no other legal site is reachable; the site is worth {worth:.1}"; here);
                return true;
            }
        }
        // `stranded-settler-leaves-the-corridor`: the early home corridor
        // (`EARLY_SETTLER_HOME_RADIUS` tiles from the one-city capital) binds
        // for the Settler's whole life. When every legal site inside it is
        // taken, wet or impassable, the exhausted search finds nothing and the
        // Settler holds the pipeline: live King civvis-20261005T203103Z
        // (game 160) stood its second Settler six tiles from Bogotá from turn
        // 29 to 66, twelve turns of them walking back into the corridor, and
        // the empire held two cities at turn 60. A stranded Settler is the
        // exhausted state, so the corridor drops here and the next turn's
        // search runs as any Settler's.
        if self.stranded_settler_leaves_the_corridor
            && self.early_settler_homes.remove(&uid).is_some()
        {
            think!(self.journal(), Expansion, Decision, "Settler leaves its early corridor at {here:?}";
                   "no legal site is reachable within {EARLY_SETTLER_HOME_RADIUS} tiles of its \
                    capital and none here; it searches as any Settler from next turn"; here);
            self.settler_stranded_at.insert(uid, (here, g.turn));
            return false;
        }
        think!(self.journal(), Expansion, Detail, "Settler is stranded at {here:?}";
               "no legal site is reachable and a city cannot be founded here; it holds until \
                the board changes"; here);
        self.settler_stranded_at.insert(uid, (here, g.turn));
        false
    }

    /// Reveal nearby dry frontier when no city target survived the exhausted
    /// search. `settler_stranded` is intentionally allowed to hold when the
    /// board has no legal city site, but that used to make the outer watchdog
    /// unreachable: it removes the target before returning, and the watchdog
    /// treated a missing target as a reason not to run. On the live bridge a
    /// fogged Loyalty horizon can therefore park several Settlers forever even
    /// though one safe movement would expose the next site.
    ///
    /// This is deliberately narrower than ordinary exploration. It is only
    /// reached by `settler-never-idles` after its patience expires, asks only
    /// for unexplored, land-traversable plots inside the exhaustion radius,
    /// respects each Settler's retired-site and blocked-plot memory, and uses
    /// the watchdog's exact visible-threat test before applying one step.
    /// Fully explored boards and boards where every frontier tile is retired
    /// remain a named stranded hold, preserving the existing safety behavior.
    pub(super) fn settler_frontier_step(&self, g: &mut Game, pid: usize, uid: u32) -> bool {
        let Some(unit) = g.units.get(&uid) else {
            return false;
        };
        if unit.moves_left <= 0.0 {
            return false;
        }
        let current = unit.pos;
        let goals: HashSet<Pos> = {
            let _memo = g.query_memo();
            g.wdisk(current, STRANDED_FRONTIER_RADIUS)
                .into_iter()
                .filter(|pos| {
                    !g.players[pid].explored.contains(pos)
                        && !g.blocked_city_sites.contains(pos)
                        && !self.settler_site_is_dead(uid, *pos)
                        && g.map.get(*pos).is_some_and(|tile| !g.rules.is_water(tile))
                        && g.unit_can_traverse(uid, *pos)
                })
                .collect()
        };
        let Some(next) = g
            .route_step_to_any(uid, &goals)
            .filter(|next| g.can_move(uid, *next))
        else {
            return false;
        };
        let reach = self.barbarian_reach(g, pid, current, REACH_SCAN_RADIUS);
        if !self.watchdog_tile_is_safe(g, pid, uid, next, &reach) {
            think!(self.journal(), Expansion, Detail,
                   "Settler holds before exploring the frontier";
                   "the nearest unexplored land is unsafe to enter this turn; it keeps its \
                    current tile until the visible threat changes"; current);
            return false;
        }
        if !self.base.path_move(g, pid, uid, next) {
            return false;
        }
        think!(self.journal(), Expansion, Detail, "Settler explores the frontier";
               "no legal city site was reachable, so it steps toward unseen land at {next:?} \
                and will re-price sites after the next board reveal"; next);
        true
    }

    /// The watchdog's one rule: a tile is safe to end the turn on when no
    /// visible hostile can reach it next turn — the barbarian reach flood,
    /// and the movement allowance of every visible at-war major unit.
    fn watchdog_tile_is_safe(
        &self,
        g: &Game,
        pid: usize,
        uid: u32,
        tile: Pos,
        reach: &BarbarianReach,
    ) -> bool {
        if !self.civilian_safe_at(g, pid, uid, tile, reach) {
            return false;
        }
        if g.city_at(tile)
            .is_some_and(|cid| g.cities[&cid].owner == pid)
        {
            return true;
        }
        let visible = self.battlefront_visibility(g, pid);
        !g.units.values().any(|unit| {
            if unit.owner == pid
                || g.players[unit.owner].is_barbarian
                || !g.is_at_war(pid, unit.owner)
                || !g.sees(&visible, unit.pos)
                || !g.unit_visible_to(unit.id, pid)
            {
                return false;
            }
            let spec = &g.rules.units[unit.kind];
            spec.class == "military"
                && !matches!(spec.domain.as_deref(), Some("sea" | "air"))
                && g.wdist(unit.pos, tile) <= spec.moves.ceil() as i32
        })
    }

    /// A Settler past its patience marches: toward its target if it holds a
    /// legal one, else toward what the exhaustion search finds, onto the
    /// first progressing tile the exact-reach rule allows. If no city target
    /// survives, the outer watchdog asks the bounded frontier recovery to
    /// reveal nearby land. Founds when it stands on its target. Returns
    /// whether it acted.
    /// Neighbours worth giving ground for, best first, when raiders already
    /// cover the tile the settler stands on. Empty whenever holding is not the
    /// losing move — nothing covers this tile, or nothing reachable beats it.
    ///
    /// Ordered by the fewest raiders that reach it, then the farthest from any
    /// of them. ⚠ It deliberately ignores PROGRESS, which every other step
    /// filter in this crate requires (`wdist(next, target) < distance` above,
    /// `regress <= 0` in `settler_step_out_of_reach`). Retreat is correct only
    /// here, because this is reached solely when standing still is losing.
    fn cornered_retreats(
        &self,
        g: &Game,
        uid: u32,
        current: Pos,
        reach: &BarbarianReach,
    ) -> Vec<Pos> {
        let here_covering = reach.raiders_covering(g, current);
        // Reach alone is not capture risk: an owned city or a surviving bound
        // guard protects this tile. Keep the normal safe-progress search above
        // this fallback, but never "escape" protection into an exposed neighbour.
        if here_covering == 0 || self.civilian_safe_at(g, g.units[&uid].owner, uid, current, reach)
        {
            return Vec::new();
        }
        let here_nearest = reach.nearest(g, current);
        let mut retreats: Vec<(usize, i32, Pos)> = g
            .nbrs(current)
            .into_iter()
            .filter(|next| g.map.get(*next).is_some() && g.can_move(uid, *next))
            .map(|next| {
                (
                    reach.raiders_covering(g, next),
                    reach.nearest(g, next),
                    next,
                )
            })
            .filter(|(covering, nearest, _)| {
                *covering < here_covering || (*covering == here_covering && *nearest > here_nearest)
            })
            .collect();
        retreats.sort_by(|a, b| {
            a.0.cmp(&b.0)
                .then_with(|| b.1.cmp(&a.1))
                .then_with(|| a.2.cmp(&b.2))
        });
        retreats.into_iter().map(|(_, _, pos)| pos).collect()
    }

    pub(super) fn settler_watchdog_step(&mut self, g: &mut Game, pid: usize, uid: u32) -> bool {
        let current = g.units[&uid].pos;
        if g.units[&uid].moves_left <= 0.0 {
            return false;
        }
        if let Some(home) = self.early_settler_homeward_target(g, uid) {
            return self.return_early_settler_home(g, pid, uid, home);
        }
        let cached = self.settler_targets.get(&uid).copied().filter(|target| {
            self.early_settler_site_allowed(g, pid, uid, *target)
                && !self.settler_site_is_dead(uid, *target)
                && (!self.settler_threat_detour_on()
                    || !self.settler_threat_deferrals.contains_key(target))
                && !self.settler_target_reserved_by_other(g, pid, uid, *target)
                && ((*target == current && g.can_found_city(uid))
                    || (*target != current
                        && self.base.valid_settle_site(g, pid, *target)
                        && !g.blocked_city_sites.contains(target)
                        && g.route_step(uid, *target, 0).is_some()))
        });
        let target = match cached {
            Some(target) => target,
            None => match self.settler_exhaustion_target(g, pid, uid) {
                Some(site) => {
                    self.settler_targets.insert(uid, site);
                    self.settler_relaxed_targets.insert(uid, site);
                    self.settler_stalls.remove(&uid);
                    self.settler_closest.remove(&uid);
                    site
                }
                None => return self.settler_stranded(g, pid, uid),
            },
        };
        if target == current {
            if !self.early_settler_site_allowed(g, pid, uid, current) {
                let home = self
                    .early_settler_homeward_target(g, uid)
                    .expect("an out-of-corridor watchdog target has an early home");
                return self.return_early_settler_home(g, pid, uid, home);
            }
            if self.science_targeted(g) {
                if let Some(why) = self.settle_site_loyalty_verdict(g, pid, current) {
                    think!(self.journal(), Expansion, Detail,
                           "Settler abandons loyalty-doomed watchdog arrival at {current:?}";
                           "{why}, so the Science lane retires the site before founding";
                           current);
                    self.settler_dead_sites.entry(uid).or_default().insert(
                        current,
                        g.turn + g.standard_duration(SETTLER_DEAD_SITE_AVOID_TURNS),
                    );
                    self.settler_targets.remove(&uid);
                    self.settler_relaxed_targets.remove(&uid);
                    return false;
                }
            }
            let worth = self.settle_value(g, pid, current);
            self.settler_targets.remove(&uid);
            let founded = g.apply(pid, &Action::FoundCity { unit: uid }).is_ok();
            if founded {
                think!(self.journal(), Expansion, Decision,
                       "Founding a city at {current:?} after the watchdog's march";
                       "the site is worth {worth:.1}"; current);
            }
            return founded;
        }
        // A linked Settler has no Move action of its own; the formation
        // that was to carry it is the thing that has not moved.
        if g.units[&uid].linked_to.is_some() {
            let _ = g.apply(pid, &Action::UnlinkUnits { unit: uid });
        }
        let reach = self.barbarian_reach(g, pid, current, REACH_SCAN_RADIUS);
        let distance = g.wdist(current, target);
        let mut steps: Vec<Pos> = Vec::new();
        if let Some(next) = g.route_step(uid, target, 0) {
            steps.push(next);
        }
        let mut progressing: Vec<Pos> = g
            .nbrs(current)
            .into_iter()
            .filter(|next| !steps.contains(next) && g.wdist(*next, target) < distance)
            .collect();
        progressing.sort_by_key(|next| (g.wdist(*next, target), *next));
        steps.extend(progressing);
        let streak = self.settler_idle_streak(uid);
        for next in steps {
            if !g.can_move(uid, next) || !self.watchdog_tile_is_safe(g, pid, uid, next, &reach) {
                continue;
            }
            if self.base.path_move(g, pid, uid, next) {
                think!(self.journal(), Expansion, Detail,
                       "Settler stops waiting and marches to {next:?}";
                       "it stood still {streak} turns; {target:?} is {distance} tiles away and \
                        nothing visible can reach {next:?} next turn"; next);
                self.settler_stalls.remove(&uid);
                return true;
            }
        }
        // ⚠⚠⚠ A HOLD IS NOT SAFETY WHEN THE TILE UNDER THE SETTLER IS ALREADY
        // COVERED. Every candidate above must both make PROGRESS
        // (`wdist(next, target) < distance`) and pass `watchdog_tile_is_safe`,
        // which is an absolute test. A settler that raiders have surrounded has
        // no such tile, so the watchdog refused "to walk it into a capture"
        // while it was standing in one — and the comparison it never made was
        // against staying.
        //
        // Live, run `civvis-20260830T095742Z`, settler 589826 — its own journal:
        //
        //     t36 settler holds inside a barbarian's reach | 3 raider(s) could
        //         take (14, 9) and no reachable tile is better
        //     t37 Settler holds at (13, 9) ... it has stood still 7 turns;
        //         the watchdog will not walk it into a capture
        //
        // It held from t34 to t47 — seventeen `settler_barbarian_combat_capture_hold`
        // events — and a horseman took it on t47. That game built EIGHT settlers
        // and finished turn 150 with ONE city, abandoned at 0.277 of the leader.
        //
        // So when raiders already cover this tile, give ground: the least-covered
        // neighbour that is STRICTLY better than here, progress or not. Bounded
        // by its own condition — coverage strictly decreases, so it cannot
        // oscillate, and once nothing covers the settler the ordinary march
        // resumes. ⚠ Retreat is exactly what the sidestep filters elsewhere
        // forbid (`regress <= 0`, `wdist < distance`); it is correct only
        // because this branch is reached solely when standing still is losing.
        let here_covering = reach.raiders_covering(g, current);
        for next in self.cornered_retreats(g, uid, current, &reach) {
            if self.base.path_move(g, pid, uid, next) {
                think!(self.journal(), Expansion, Detail,
                       "Settler gives ground rather than wait to be taken";
                       "it stood still {streak} turns and {here_covering} raider(s) already \
                        cover {current:?}; {next:?} is reached by fewer";
                       next);
                self.settler_stalls.remove(&uid);
                return true;
            }
        }
        think!(self.journal(), Expansion, Detail,
               "Settler holds at {current:?}: every step toward {target:?} is in a hostile's reach";
               "it has stood still {streak} turns; the watchdog will not walk it into a capture";
               current);
        false
    }
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod walk_price_tests;
