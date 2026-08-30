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
//!    (`settler_exhaustion_target`, tier 2); then any legal reachable site at
//!    all, nearest first (tier 3). Failing both it founds where it stands if
//!    the engine allows, and otherwise says so in the journal — a Settler that
//!    holds is never silent.
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
use super::{AdvancedAi, SETTLEMENT_GLOBAL_PREFILTER_LIMIT, SETTLER_STEP_RISK_LIMIT};
use crate::ai::BasicAi;
use crate::game::{Action, Game};
use crate::think;
use crate::Pos;

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

    /// The arrival verdict for a site the exhaustion search chose: the fog
    /// guesses had their say when the walk began and may not refuse the
    /// founding at its end; only the engine's own Loyalty calculation, at
    /// the same [`STRANDED_SITE_MIN_HOLD_TURNS`] floor the choice used, may.
    /// Without this a relaxed target was refused on arrival by the strict
    /// verdict, retired, chosen again by the next exhaustion search, walked
    /// to again — 1,062 idle turns of that loop in eight live-genome games.
    pub(super) fn relaxed_arrival_verdict(g: &Game, pid: usize, site: Pos) -> Option<String> {
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

    /// Why the exhaustion search may not take `site` under
    /// `exhaustion-loyalty-guard`. The preferred search refuses a site within
    /// five tiles of a met major's border whose city it has not seen
    /// (`beside_unresolved_major_border`), because the forecast sums the
    /// cities on the board and that one is not on it. The exhaustion search
    /// set that refusal aside as a fog guess; on the live board it is the one
    /// guess that is right. Of the 25 exhaustion foundings in the King runs
    /// of 2026-08-27..29, six revolted (24%, against 2 of 387 preferred
    /// foundings), three of them at −22 Loyalty a turn from their first
    /// reading — the settler handed a city to the rival that pressed it.
    /// `None` with the gene off, with `frontier-loyalty` off, or when no
    /// border hides a city.
    pub(super) fn exhaustion_site_unpriceable(&self, g: &Game, site: Pos) -> Option<&'static str> {
        (self.exhaustion_loyalty_guard
            && self.frontier_loyalty
            && Self::beside_unresolved_major_border(g, site))
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

    /// Drop the sites `exhaustion_site_unpriceable` refuses, and the sites
    /// inside a rival major's sphere, from a candidate list, saying so once.
    /// Nothing is dropped with the gene off.
    fn set_aside_unpriceable_sites(&self, g: &Game, pid: usize, candidates: &mut Vec<(Pos, f64)>) {
        if !self.exhaustion_loyalty_guard {
            return;
        }
        let before = candidates.len();
        candidates.retain(|(pos, _)| self.exhaustion_site_unpriceable(g, *pos).is_none());
        let unpriceable = before - candidates.len();
        candidates.retain(|(pos, _)| !Self::inside_rival_sphere(g, pid, *pos));
        let in_sphere = before - unpriceable - candidates.len();
        if unpriceable > 0 {
            think!(self.journal(), Expansion, Detail,
                   "Stranded Settler sets aside {unpriceable} unpriceable site(s)";
                   "each lies within five tiles of a rival border whose city may be hidden; \
                    the forecast cannot price that Loyalty pressure, and the exhaustion \
                    search will not guess at a city it would hand to that rival");
        }
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
    /// revolts inside [`STRANDED_SITE_MIN_HOLD_TURNS`]. Tier 3: any legal
    /// reachable site, nearest first, with the same per-Settler exclusions.
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
            .filter(|(pos, _)| {
                !g.blocked_city_sites.contains(pos)
                    && !self.settler_site_is_dead(uid, *pos)
                    && Some(*pos) != avoided
                    && (!self.settler_threat_detour_on()
                        || !self.settler_threat_deferrals.contains_key(pos))
                    && !self.settler_target_reserved_by_other(g, pid, uid, *pos)
            })
            .collect();
        self.set_aside_unpriceable_sites(g, pid, &mut ranked);
        for _ in 0..=STRANDED_FORECAST_RETRIES {
            let Some((site, value)) = BasicAi::first_reachable_settle_site(g, uid, &ranked) else {
                break;
            };
            match Self::settle_site_forecast_revolt(g, pid, site) {
                Some((per_turn, turns)) if turns < STRANDED_SITE_MIN_HOLD_TURNS => {
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
        let live_fallback_loyalty =
            live_fallback_safety && (self.base.loyalty_rate_alarm || self.frontier_loyalty);
        let visible = live_fallback_safety.then(|| self.battlefront_visibility(g, pid));
        let mut legal: Vec<(Pos, f64)> = g
            .wdisk(from, radius)
            .into_iter()
            .filter(|pos| {
                self.base.valid_settle_site(g, pid, *pos)
                    && !g.blocked_city_sites.contains(pos)
                    && !self.settler_site_is_dead(uid, *pos)
                    && Some(*pos) != avoided
                    && (!self.settler_threat_detour_on()
                        || !self.settler_threat_deferrals.contains_key(pos))
                    && !self.settler_target_reserved_by_other(g, pid, uid, *pos)
            })
            .filter_map(|pos| {
                if !live_fallback_safety {
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
        if self.exhaustion_loyalty_guard {
            // This tier ran no forecast at all: of the sixteen nearest-legal
            // foundings in the King runs of 2026-08-27..29, six read a
            // negative Loyalty rate at their first reading and three
            // revolted. Under the gene it pays the same concrete-revolt check
            // as the ranked tier, retries included; a Settler that then holds
            // is named by `settler_stranded`.
            for _ in 0..=STRANDED_FORECAST_RETRIES {
                let (site, _) = BasicAi::first_reachable_settle_site(g, uid, &legal)?;
                match Self::settle_site_forecast_revolt(g, pid, site) {
                    Some((per_turn, turns)) if turns < STRANDED_SITE_MIN_HOLD_TURNS => {
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
        self.settler_targets.remove(&uid);
        self.settler_stalls.remove(&uid);
        self.settler_closest.remove(&uid);
        if g.can_found_city(uid)
            && self.exhaustion_site_unpriceable(g, here).is_none()
            && !(self.exhaustion_loyalty_guard && Self::inside_rival_sphere(g, pid, here))
            && !Self::settle_site_forecast_revolt(g, pid, here)
                .is_some_and(|(_, turns)| turns < STRANDED_SITE_MIN_HOLD_TURNS)
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
        think!(self.journal(), Expansion, Detail, "Settler is stranded at {here:?}";
               "no legal site is reachable and a city cannot be founded here; it holds until \
                the board changes"; here);
        self.settler_stranded_at.insert(uid, (here, g.turn));
        false
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
    /// first progressing tile the exact-reach rule allows. Founds when it
    /// stands on its target. Returns whether it acted.
    pub(super) fn settler_watchdog_step(&mut self, g: &mut Game, pid: usize, uid: u32) -> bool {
        let current = g.units[&uid].pos;
        if g.units[&uid].moves_left <= 0.0 {
            return false;
        }
        let cached = self.settler_targets.get(&uid).copied().filter(|target| {
            !self.settler_site_is_dead(uid, *target)
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
        think!(self.journal(), Expansion, Detail,
               "Settler holds at {current:?}: every step toward {target:?} is in a hostile's reach";
               "it has stood still {streak} turns; the watchdog will not walk it into a capture";
               current);
        false
    }
}

#[cfg(test)]
mod tests {
    use super::super::AdvancedAi;
    use super::*;
    use crate::ai::Ai;
    use crate::game::Game;
    use crate::reasoning::Journal;

    /// The gene ships off in both stock controllers; the toggles are twins.
    #[test]
    fn the_gene_is_off_in_both_controllers_and_the_toggles_are_twins() {
        let fresh = AdvancedAi::new();
        let legacy = AdvancedAi::legacy();
        assert!(!fresh.settler_never_idles);
        assert!(!legacy.settler_never_idles);
        let mut opted = AdvancedAi::new();
        opted.enable_settler_never_idles();
        assert!(opted.settler_never_idles);
        opted.disable_settler_never_idles();
        assert!(!opted.settler_never_idles);
    }

    /// The guard ships off in both stock controllers; the toggles are twins.
    #[test]
    fn the_guard_is_off_in_both_controllers_and_the_toggles_are_twins() {
        let fresh = AdvancedAi::new();
        let legacy = AdvancedAi::legacy();
        assert!(!fresh.exhaustion_loyalty_guard);
        assert!(!legacy.exhaustion_loyalty_guard);
        let mut opted = AdvancedAi::new();
        opted.enable_exhaustion_loyalty_guard();
        assert!(opted.exhaustion_loyalty_guard);
        opted.disable_exhaustion_loyalty_guard();
        assert!(!opted.exhaustion_loyalty_guard);
    }

    /// The live failure of `civvis-20260829T030044Z` t70: every preferred
    /// site was refused "within five tiles of a rival border whose city may
    /// be hidden", the exhaustion search took (15, 14) anyway, and the city
    /// read −22 Loyalty a turn from its first turn and revolted seven turns
    /// later. Under the guard the same board yields no exhaustion target and
    /// the Settler is named stranded rather than founding; off, the search
    /// still takes a site.
    #[test]
    fn exhaustion_sets_aside_sites_beside_an_unresolved_rival_border() {
        for guard in [false, true] {
            let mut g = Game::new_full(1, 20, 12, 91_306, 120, 0, false);
            let settler = g
                .player_unit_ids(0)
                .into_iter()
                .find(|uid| g.units[uid].kind == "settler")
                .expect("a starting settler");
            g.units.get_mut(&settler).expect("settler").moves_left = 20.0;
            let explored: Vec<Pos> = g.map.tiles.keys().copied().collect();
            g.players[0].explored.extend(explored.iter().copied());
            let mut ai = AdvancedAi::new();
            ai.enable_engine_repairs();
            ai.enable_settler_never_idles();
            ai.enable_frontier_loyalty();
            if guard {
                ai.enable_exhaustion_loyalty_guard();
            } else {
                ai.disable_exhaustion_loyalty_guard();
            }
            ai.attach_journal(Journal::recording());
            assert!(
                ai.settler_exhaustion_target(&g, 0, settler).is_some(),
                "fixture: the open board offers an exhaustion site"
            );
            // A met major's border the mirror could not attribute to a seen
            // city, on every plot: the whole board is five tiles from one.
            g.unseen_major_borders.extend(explored.iter().copied());
            let picked = ai.settler_exhaustion_target(&g, 0, settler);
            if guard {
                assert_eq!(picked, None, "every site lies beside an unresolved border");
                assert!(
                    !ai.settler_stranded(&mut g, 0, settler),
                    "the stranded Settler does not found on unpriceable ground"
                );
                assert!(g.cities.is_empty(), "no city was founded");
                assert!(
                    ai.settler_stranded_at.contains_key(&settler),
                    "the hold is named, not silent"
                );
            } else {
                assert!(
                    picked.is_some(),
                    "off, the search is unchanged and still takes a site"
                );
            }
        }
    }

    /// The rival-sphere sieve on the doomed-target board: the plot four tiles
    /// from a pop-12 rival capital is inside the sphere, a plot beside our
    /// own capital is not, and with both plots the only ones left the guard's
    /// exhaustion search answers the home plot instead of spending its
    /// forecast retries on the doomed one and returning nothing.
    #[test]
    fn the_sieve_keeps_the_forecast_for_plots_outside_the_rivals_sphere() {
        let mut g = Game::new_full(2, 40, 24, 91_775, 250, 0, false);
        g.current = 0;
        for pid in 0..2 {
            let settler = g
                .player_unit_ids(pid)
                .into_iter()
                .find(|unit| g.units[unit].kind == "settler")
                .expect("a starting settler");
            let pos = g.units[&settler].pos;
            g.remove_unit(settler);
            g.found_city_for(pid, pos, None);
        }
        for unit in g.player_unit_ids(0) {
            g.remove_unit(unit);
        }
        let ours = g.player_city_ids(0)[0];
        let theirs = g.player_city_ids(1)[0];
        let home = g.cities[&ours].pos;
        let rival = g.cities[&theirs].pos;
        assert!(g.wdist(home, rival) >= 12, "fixture needs a distant rival");
        let positions: Vec<Pos> = g.map.tiles.keys().copied().collect();
        for position in &positions {
            let tile = g.map.tiles.get_mut(position).unwrap();
            tile.terrain = crate::name!("grassland");
            tile.feature = None;
            tile.hills = false;
            tile.resource = None;
            tile.improvement = None;
            tile.district = None;
            tile.wonder = None;
            g.players[0].explored.insert(*position);
        }
        g.cities.get_mut(&theirs).unwrap().pop = 12;
        g.cities.get_mut(&ours).unwrap().pop = 6;
        let mut beside_rival: Vec<Pos> = positions
            .iter()
            .copied()
            .filter(|pos| g.wdist(*pos, rival) == 4 && g.wdist(*pos, home) >= 4)
            .collect();
        beside_rival.sort_unstable();
        let doomed = beside_rival[0];
        // Beside home, out of the rival's reach, and inside the stranded
        // search radius from where the Settler will stand.
        let mut beside_home: Vec<Pos> = positions
            .iter()
            .copied()
            .filter(|pos| {
                g.wdist(*pos, home) <= 4
                    && g.wdist(*pos, rival) >= 8
                    && g.wdist(*pos, doomed) < STRANDED_SITE_RADIUS
            })
            .collect();
        beside_home.sort_by_key(|pos| (g.wdist(*pos, doomed), *pos));
        let mut ai = AdvancedAi::new();
        ai.enable_engine_repairs();
        ai.enable_settler_never_idles();
        ai.enable_exhaustion_loyalty_guard();
        ai.attach_journal(Journal::recording());
        let safe = beside_home
            .iter()
            .copied()
            .find(|pos| ai.base.valid_settle_site(&g, 0, *pos))
            .expect("fixture: a legal plot beside home");
        assert!(
            AdvancedAi::inside_rival_sphere(&g, 0, doomed),
            "four tiles from a rival capital"
        );
        assert!(
            !AdvancedAi::inside_rival_sphere(&g, 0, safe),
            "beside our own capital"
        );
        // The Settler stands beside the doomed plot, the nearest legal one.
        let start = g
            .nbrs(doomed)
            .into_iter()
            .find(|pos| g.map.tiles.contains_key(pos) && g.wdist(*pos, rival) >= 4)
            .expect("a neighbour to stand on");
        let settler = g.spawn_test_unit("settler", 0, start);
        g.units.get_mut(&settler).expect("settler").moves_left = 40.0;
        let retired = ai.settler_dead_sites.entry(settler).or_default();
        for pos in &positions {
            if *pos != doomed && *pos != safe {
                retired.insert(*pos, g.turn + 1000);
            }
        }
        assert_eq!(
            ai.settler_exhaustion_target(&g, 0, settler),
            Some(safe),
            "the sieve drops the doomed plot and the forecast passes the home plot"
        );
    }

    /// Arretium in `civvis-20260829T090147Z`: a plot four tiles from a small
    /// visible rival city and five from our own capital passes the mirror's
    /// forecast (the rival's unseen cities are not on the board) and revolted
    /// in peacetime. Under the guard the preferred search's verdict refuses it
    /// for standing inside the rival's sphere; off, the forecast alone speaks.
    #[test]
    fn the_preferred_verdict_refuses_a_plot_inside_a_rivals_sphere_under_the_guard() {
        let mut g = Game::new_full(2, 40, 24, 91_775, 250, 0, false);
        g.current = 0;
        for pid in 0..2 {
            let settler = g
                .player_unit_ids(pid)
                .into_iter()
                .find(|unit| g.units[unit].kind == "settler")
                .expect("a starting settler");
            let pos = g.units[&settler].pos;
            g.remove_unit(settler);
            g.found_city_for(pid, pos, None);
        }
        for unit in g.player_unit_ids(0) {
            g.remove_unit(unit);
        }
        let ours = g.player_city_ids(0)[0];
        let theirs = g.player_city_ids(1)[0];
        let home = g.cities[&ours].pos;
        let rival = g.cities[&theirs].pos;
        assert!(g.wdist(home, rival) >= 12, "fixture needs a distant rival");
        let positions: Vec<Pos> = g.map.tiles.keys().copied().collect();
        for position in &positions {
            let tile = g.map.tiles.get_mut(position).unwrap();
            tile.terrain = crate::name!("grassland");
            tile.feature = None;
            tile.hills = false;
            tile.resource = None;
            tile.improvement = None;
            tile.district = None;
            tile.wonder = None;
            g.players[0].explored.insert(*position);
        }
        // A small rival city: the forecast reads its two citizens as harmless
        // once a city of ours stands five tiles from the plot — Arretium's
        // board: Rome five away, Aksu (pop 2) four away.
        g.cities.get_mut(&theirs).unwrap().pop = 2;
        g.cities.get_mut(&ours).unwrap().pop = 6;
        let mut beside_rival: Vec<Pos> = positions
            .iter()
            .copied()
            .filter(|pos| g.wdist(*pos, rival) == 4 && g.wdist(*pos, home) > 4)
            .collect();
        beside_rival.sort_unstable();
        let plot = beside_rival[0];
        let mut near_plot: Vec<Pos> = positions
            .iter()
            .copied()
            .filter(|pos| g.wdist(*pos, plot) == 5 && g.wdist(*pos, rival) >= 6)
            .collect();
        near_plot.sort_unstable();
        let second = g.found_city_for(0, near_plot[0], None);
        g.cities.get_mut(&second).unwrap().pop = 6;
        assert!(
            AdvancedAi::settle_site_forecast_revolt(&g, 0, plot).is_none(),
            "fixture: the forecast passes a plot beside a pop-2 rival city"
        );
        assert!(AdvancedAi::inside_rival_sphere(&g, 0, plot));
        let mut ai = AdvancedAi::new();
        ai.enable_engine_repairs();
        assert_eq!(
            ai.settle_site_loyalty_verdict(&g, 0, plot),
            None,
            "off, the forecast alone speaks and passes it"
        );
        ai.enable_exhaustion_loyalty_guard();
        let why = ai
            .settle_site_loyalty_verdict(&g, 0, plot)
            .expect("under the guard the preferred verdict refuses the plot");
        assert!(why.contains("rival major's city"), "{why}");
    }

    /// The nearest-legal tier founded with no forecast at all. The board of
    /// `research_probe`'s doomed-target tests: a pop-12 rival capital and a
    /// plot four tiles from it where a new city loses twenty Loyalty a turn.
    /// Every other plot is retired for this Settler, so the ranked tier has
    /// nothing and the nearest tier answers. Off, it takes the doomed plot;
    /// under the guard it forecasts, refuses, and returns nothing.
    #[test]
    fn the_nearest_legal_tier_forecasts_under_the_guard() {
        for guard in [false, true] {
            let mut g = Game::new_full(2, 40, 24, 91_775, 250, 0, false);
            g.current = 0;
            for pid in 0..2 {
                let settler = g
                    .player_unit_ids(pid)
                    .into_iter()
                    .find(|unit| g.units[unit].kind == "settler")
                    .expect("a starting settler");
                let pos = g.units[&settler].pos;
                g.remove_unit(settler);
                g.found_city_for(pid, pos, None);
            }
            for unit in g.player_unit_ids(0) {
                g.remove_unit(unit);
            }
            let ours = g.player_city_ids(0)[0];
            let theirs = g.player_city_ids(1)[0];
            let home = g.cities[&ours].pos;
            let rival = g.cities[&theirs].pos;
            assert!(g.wdist(home, rival) >= 12, "fixture needs a distant rival");
            let positions: Vec<Pos> = g.map.tiles.keys().copied().collect();
            for position in &positions {
                let tile = g.map.tiles.get_mut(position).unwrap();
                tile.terrain = crate::name!("grassland");
                tile.feature = None;
                tile.hills = false;
                tile.resource = None;
                tile.improvement = None;
                tile.district = None;
                tile.wonder = None;
                g.players[0].explored.insert(*position);
            }
            g.cities.get_mut(&theirs).unwrap().pop = 12;
            g.cities.get_mut(&ours).unwrap().pop = 6;
            let mut beside_rival: Vec<Pos> = positions
                .iter()
                .copied()
                .filter(|pos| g.wdist(*pos, rival) == 4 && g.wdist(*pos, home) >= 4)
                .collect();
            beside_rival.sort_unstable();
            let doomed = beside_rival[0];
            let start = g
                .nbrs(doomed)
                .into_iter()
                .find(|pos| g.map.tiles.contains_key(pos) && g.wdist(*pos, rival) >= 4)
                .expect("a neighbour to stand on");
            let settler = g.spawn_test_unit("settler", 0, start);
            g.units.get_mut(&settler).expect("settler").moves_left = 20.0;
            let mut ai = AdvancedAi::new();
            ai.enable_engine_repairs();
            ai.enable_settler_never_idles();
            if guard {
                ai.enable_exhaustion_loyalty_guard();
            } else {
                ai.disable_exhaustion_loyalty_guard();
            }
            ai.attach_journal(Journal::recording());
            assert!(
                ai.base.valid_settle_site(&g, 0, doomed),
                "fixture: the doomed plot is a legal site"
            );
            let (per_turn, turns) = AdvancedAi::settle_site_forecast_revolt(&g, 0, doomed)
                .expect("fixture: the forecast dooms the plot");
            assert!(
                turns < STRANDED_SITE_MIN_HOLD_TURNS,
                "fixture: revolt in {turns:.0} turns at {per_turn:.1} a turn"
            );
            // Retire every other plot for this Settler, so only the nearest
            // tier can answer.
            let retired = ai.settler_dead_sites.entry(settler).or_default();
            for pos in &positions {
                if *pos != doomed {
                    retired.insert(*pos, g.turn + 1000);
                }
            }
            let picked = ai.settler_exhaustion_target(&g, 0, settler);
            if guard {
                assert_eq!(
                    picked, None,
                    "the guard forecasts the nearest legal site and refuses the revolt"
                );
            } else {
                assert_eq!(
                    picked,
                    Some(doomed),
                    "off, the nearest tier takes the doomed plot unasked"
                );
            }
        }
    }

    /// A Settler that never moves counts up; one that moves resets; a
    /// second look in the same turn does not count twice.
    #[test]
    fn the_idle_streak_counts_turns_on_one_tile_once_each() {
        let mut g = Game::new_full(1, 12, 8, 91_301, 60, 0, false);
        let settler = g
            .player_unit_ids(0)
            .into_iter()
            .find(|uid| g.units[uid].kind == "settler")
            .expect("a starting settler");
        let mut ai = AdvancedAi::new();
        ai.enable_settler_never_idles();
        assert_eq!(ai.note_settler_idle(&g, settler), 0);
        assert_eq!(
            ai.note_settler_idle(&g, settler),
            0,
            "same turn, same answer"
        );
        g.turn += 1;
        assert_eq!(ai.note_settler_idle(&g, settler), 1);
        g.turn += 1;
        assert_eq!(ai.note_settler_idle(&g, settler), 2);
        let here = g.units[&settler].pos;
        let step = g
            .nbrs(here)
            .into_iter()
            .find(|pos| g.map.get(*pos).is_some_and(|tile| !g.rules.is_water(tile)))
            .expect("a land neighbour");
        g.units.get_mut(&settler).expect("settler").pos = step;
        g.turn += 1;
        assert_eq!(
            ai.note_settler_idle(&g, settler),
            0,
            "moving resets the streak"
        );
    }

    /// On a board where every site is explicitly retired, neither controller
    /// resurrects one. The live safety gene still names the stranded hold
    /// instead of wandering back through the same rejected corridor.
    ///
    /// The board: one city, one Settler standing in it, every plot within
    /// three tiles of the city legal-but-excluded by spacing, the rest of the
    /// map retired for this Settler as `settler_dead_sites` — exactly the
    /// state thirty-turn retirements leave a live seat in after a handful of
    /// forecast refusals.
    #[test]
    fn a_settler_whose_every_site_was_retired_walks_under_the_gene_and_holds_without_it() {
        for gene in [false, true] {
            let mut g = Game::new_full(1, 16, 10, 91_302, 120, 0, false);
            let founding = g
                .player_unit_ids(0)
                .into_iter()
                .find(|uid| g.units[uid].kind == "settler")
                .expect("a starting settler");
            g.apply(0, &Action::FoundCity { unit: founding })
                .expect("the starting settler founds");
            let home = g.cities.values().next().expect("a city").pos;
            let settler = g.spawn_test_unit("settler", 0, home);
            g.units.get_mut(&settler).expect("settler").moves_left = 2.0;
            let mut ai = AdvancedAi::new();
            ai.enable_engine_repairs();
            ai.enable_loyalty_rate_alarm();
            // The ledger pins the gene on; the off arm withholds it by name.
            if gene {
                ai.enable_settler_never_idles();
            } else {
                ai.disable_settler_never_idles();
            }
            ai.attach_journal(Journal::recording());
            // Retire every plot for this Settler: the state the live seat
            // reaches after its forecast refusals compound.
            let all: Vec<Pos> = g.map.tiles.keys().copied().collect();
            let retired = ai.settler_dead_sites.entry(settler).or_default();
            for pos in all {
                retired.insert(pos, g.turn + 1000);
            }
            let before = g.units[&settler].pos;
            let acted = ai.advanced_settler_step(&mut g, 0, settler);
            let after = g.units[&settler].pos;
            assert!(!acted, "all explicitly retired sites remain retired");
            assert_eq!(after, before, "the settler does not wander to a dead site");
            assert!(
                !ai.settler_targets.contains_key(&settler),
                "the stranded settler carries no resurrected target"
            );
            if gene {
                assert!(
                    ai.settler_stranded_at.contains_key(&settler),
                    "the safety gene records the stranded hold"
                );
            }
        }
    }

    /// Exhaustion is a relaxed search, not permission to resurrect a target
    /// another branch deliberately retired.  This is the live parallel-settler
    /// failure: a hysteresis retirement was recorded for a peer, then the
    /// never-idles fallback ignored it and sent both Settlers back to the same
    /// site.
    #[test]
    fn exhaustion_skips_retired_and_peer_reserved_sites() {
        let mut g = Game::new_full(1, 20, 12, 91_306, 120, 0, false);
        let settler = g
            .player_unit_ids(0)
            .into_iter()
            .find(|uid| g.units[uid].kind == "settler")
            .expect("a starting settler");
        let home = g.units[&settler].pos;
        let peer = g.spawn_test_unit("settler", 0, home);
        // Give the fixture's probe Settlers enough movement to make the
        // one-turn reachability test independent of the random starting ring.
        g.units.get_mut(&settler).expect("settler").moves_left = 20.0;
        g.units.get_mut(&peer).expect("peer").moves_left = 20.0;
        // The production scan intentionally refuses unobserved opening
        // footprints. This regression is about retirement/reservation, so
        // make the fixture's map fully observed first.
        let explored: Vec<Pos> = g.map.tiles.keys().copied().collect();
        g.players[0].explored.extend(explored);

        let mut ai = AdvancedAi::new();
        ai.enable_engine_repairs();
        ai.enable_settler_never_idles();
        let reachable: Vec<Pos> = ai
            .settle_sites_with_limit(&g, 0, home, 14, Some(SETTLEMENT_GLOBAL_PREFILTER_LIMIT))
            .into_iter()
            .map(|(pos, _)| pos)
            .filter(|pos| g.path_to(settler, *pos).is_some())
            .take(4)
            .collect();
        let direct_valid = g
            .wdisk(home, 14)
            .into_iter()
            .filter(|pos| ai.base.valid_settle_site(&g, 0, *pos))
            .count();
        assert!(
            reachable.len() >= 3,
            "fixture needs three reachable city sites, got {reachable:?} (direct valid={direct_valid})"
        );
        let retired = reachable[0];
        let reserved = reachable[1];
        let avoided = reachable[2];
        ai.settler_dead_sites
            .entry(settler)
            .or_default()
            .insert(retired, g.turn + 1000);
        ai.settler_targets.insert(peer, reserved);
        ai.settler_avoid.insert(settler, (avoided, g.turn + 1000));

        let picked = ai.settler_exhaustion_target(&g, 0, settler);
        assert!(
            picked.is_some(),
            "the fixture leaves a fourth reachable site"
        );
        assert_ne!(
            picked,
            Some(retired),
            "the fallback resurrected a dead site"
        );
        assert_ne!(
            picked,
            Some(reserved),
            "the fallback duplicated a peer target"
        );
        assert_ne!(
            picked,
            Some(avoided),
            "the fallback resurrected the target in this Settler's hysteresis cooldown"
        );
    }

    /// A live fallback must not answer "never idle" by selecting a plot a
    /// visible hostile can take on the next turn.  The historical controller
    /// still asks the nearest legal site; the bridge's capture lessons reject
    /// that sole exposed target and let the emergency safety pass/stranded
    /// watchdog hold or retreat instead.
    #[test]
    fn live_exhaustion_fallback_drops_an_exposed_legal_site() {
        let mut g = Game::new_full(2, 32, 20, 91_319, 120, 0, false);
        g.current = 0;
        let founding = g
            .player_unit_ids(0)
            .into_iter()
            .find(|uid| g.units[uid].kind == "settler")
            .expect("a starting settler");
        let home = g.units[&founding].pos;
        g.apply(0, &crate::game::Action::FoundCity { unit: founding })
            .expect("the starting settler founds");
        for unit in g.player_unit_ids(0) {
            g.remove_unit(unit);
        }
        let positions: Vec<Pos> = g.map.tiles.keys().copied().collect();
        for position in &positions {
            let tile = g.map.tiles.get_mut(position).expect("map tile");
            tile.terrain = crate::name!("grassland");
            tile.feature = None;
            tile.hills = false;
            tile.resource = None;
            tile.improvement = None;
            tile.district = None;
            tile.district_foundation = None;
            tile.wonder = None;
            g.players[0].explored.insert(*position);
        }
        let probe = AdvancedAi::new();
        let danger = positions
            .iter()
            .copied()
            .filter(|pos| {
                g.wdist(home, *pos) >= 6
                    && g.wdist(home, *pos) <= 12
                    && probe.base.valid_settle_site(&g, 0, *pos)
            })
            .min()
            .expect("a legal frontier site");
        let start = g
            .nbrs(danger)
            .into_iter()
            .find(|pos| {
                g.map
                    .get(*pos)
                    .is_some_and(|tile| g.rules.is_passable(tile) && !g.rules.is_water(tile))
            })
            .expect("a passable neighbour for the Settler");
        let hostile_tile = g
            .map
            .tiles
            .keys()
            .copied()
            .find(|pos| {
                g.wdist(danger, *pos) == 1
                    && *pos != start
                    && g.map
                        .get(*pos)
                        .is_some_and(|tile| g.rules.is_passable(tile) && !g.rules.is_water(tile))
            })
            .expect("a visible hostile post beside the danger site");
        let settler = g.spawn_test_unit("settler", 0, start);
        g.units.get_mut(&settler).expect("settler").moves_left = 20.0;
        let hostile = g.spawn_test_unit("warrior", 1, hostile_tile);
        g.at_war.insert((0, 1));
        g.at_war.insert((1, 0));
        g.players[0].explored.insert(hostile_tile);

        let mut plain = AdvancedAi::new();
        plain.enable_settler_never_idles();
        plain.settler_dead_sites.entry(settler).or_default().extend(
            g.map
                .tiles
                .keys()
                .copied()
                .filter(|pos| *pos != danger)
                .map(|pos| (pos, g.turn + 1000)),
        );
        assert_eq!(
            plain.settler_exhaustion_target(&g, 0, settler),
            Some(danger),
            "the historical nearest-legal fallback sees the fixture site"
        );

        let mut live = AdvancedAi::new();
        live.enable_live_bridge_universe();
        live.enable_settler_never_idles();
        live.settler_dead_sites.entry(settler).or_default().extend(
            g.map
                .tiles
                .keys()
                .copied()
                .filter(|pos| *pos != danger)
                .map(|pos| (pos, g.turn + 1000)),
        );
        let visible = live.battlefront_visibility(&g, 0);
        assert!(g.unit_visible_to(hostile, 0) && g.sees(&visible, hostile_tile));
        assert!(
            live.settlement_tile_risk(&g, 0, Some(settler), danger, &visible)
                > super::super::SETTLER_STEP_RISK_LIMIT,
            "the fixture danger site is visibly capturable"
        );
        assert_eq!(
            live.settler_exhaustion_target(&g, 0, settler),
            None,
            "the live fallback refuses the exposed sole site"
        );
    }

    /// The watchdog respects the one rule it keeps: a tile a visible raider
    /// can reach next turn is not taken, however long the Settler has waited.
    #[test]
    fn the_watchdog_will_not_step_into_a_raiders_reach() {
        let mut g = Game::new_full(2, 16, 10, 91_303, 120, 0, false);
        let founding = g
            .player_unit_ids(0)
            .into_iter()
            .find(|uid| g.units[uid].kind == "settler")
            .expect("a starting settler");
        g.apply(0, &Action::FoundCity { unit: founding })
            .expect("the starting settler founds");
        let home = g
            .cities
            .values()
            .find(|city| city.owner == 0)
            .expect("a city")
            .pos;
        let settler = g.spawn_test_unit("settler", 0, home);
        g.units.get_mut(&settler).expect("settler").moves_left = 2.0;
        let Some(barbarian) = g.barb_pid else {
            return; // no barbarian seat on this fixture
        };
        // A warrior on every land tile two steps out: every neighbour of the
        // city is inside its reach.
        let ring: Vec<Pos> = g
            .wdisk(home, 2)
            .into_iter()
            .filter(|pos| g.wdist(*pos, home) == 2)
            .filter(|pos| g.map.get(*pos).is_some_and(|tile| !g.rules.is_water(tile)))
            .collect();
        for pos in &ring {
            g.spawn_test_unit("warrior", barbarian, *pos);
        }
        let mut ai = AdvancedAi::new();
        ai.enable_engine_repairs();
        ai.enable_settler_never_idles();
        ai.attach_journal(Journal::recording());
        let target = g
            .map
            .tiles
            .keys()
            .copied()
            .find(|pos| g.wdist(*pos, home) == 6 && ai.base.valid_settle_site(&g, 0, *pos))
            .expect("a legal site six tiles out");
        ai.settler_targets.insert(settler, target);
        ai.settler_idle_streak
            .insert(settler, (home, g.turn, SETTLER_IDLE_PATIENCE));
        let reach = ai.barbarian_reach(&g, 0, home, REACH_SCAN_RADIUS);
        let every_neighbour_covered = g
            .nbrs(home)
            .into_iter()
            .filter(|pos| g.can_move(settler, *pos))
            .all(|pos| reach.covers(&g, pos));
        if !every_neighbour_covered {
            return; // the ring did not close on this fixture; nothing to prove
        }
        assert!(!ai.settler_watchdog_step(&mut g, 0, settler));
        assert_eq!(g.units[&settler].pos, home, "it stayed in the city");
    }

    /// A site the exhaustion search chose is founded on arrival even where
    /// the strict verdict's fog guess would refuse it; the same board with
    /// the site chosen the ordinary way is still refused. The engine's own
    /// forecast is the one thing both rules keep.
    #[test]
    fn an_exhaustion_site_is_founded_on_arrival_where_the_fog_guess_would_refuse_it() {
        for relaxed in [true, false] {
            let mut g = Game::new_full(1, 20, 12, 91_305, 120, 0, false);
            let founding = g
                .player_unit_ids(0)
                .into_iter()
                .find(|uid| g.units[uid].kind == "settler")
                .expect("a starting settler");
            g.apply(0, &Action::FoundCity { unit: founding })
                .expect("the starting settler founds");
            let home = g.cities.values().next().expect("a city").pos;
            // Explore only the home halo so a far site is a fogged frontier.
            let halo: Vec<Pos> = g.wdisk(home, 5);
            g.players[0].explored.clear();
            g.players[0].explored.extend(halo);
            let far: Vec<Pos> = g
                .map
                .tiles
                .keys()
                .copied()
                .filter(|pos| g.wdist(*pos, home) >= 9)
                .collect();
            let mut site = None;
            for pos in far {
                let probe = g.spawn_test_unit("settler", 0, pos);
                let legal = g.can_found_city(probe) && AdvancedAi::beyond_loyalty_reach(&g, 0, pos);
                g.remove_unit(probe);
                if legal {
                    site = Some(pos);
                    break;
                }
            }
            let site = site.expect("fixture needs a legal fogged frontier site");
            let settler = g.spawn_test_unit("settler", 0, site);
            g.units.get_mut(&settler).expect("settler").moves_left = 2.0;
            let mut ai = AdvancedAi::new();
            ai.enable_live_bridge();
            ai.enable_loyalty_rate_alarm();
            ai.enable_settler_never_idles();
            assert!(ai.frontier_loyalty, "the live seat carries the fog guess");
            ai.settler_targets.insert(settler, site);
            if relaxed {
                ai.settler_relaxed_targets.insert(settler, site);
            }
            let cities_before = g.player_city_ids(0).len();
            let acted = ai.advanced_settler_step(&mut g, 0, settler);
            let founded = g.player_city_ids(0).len() > cities_before;
            if relaxed {
                assert!(acted && founded, "the relaxed arrival founds the colony");
            } else {
                assert!(
                    !founded,
                    "the strict arrival still refuses the fogged frontier"
                );
            }
        }
    }

    /// A Settler with no legal site anywhere but the one underfoot founds
    /// there; one with none at all says it is stranded rather than holding
    /// silently.
    #[test]
    fn a_stranded_settler_founds_where_it_stands_or_says_so() {
        let mut g = Game::new_full(1, 12, 8, 91_304, 60, 0, false);
        let settler = g
            .player_unit_ids(0)
            .into_iter()
            .find(|uid| g.units[uid].kind == "settler")
            .expect("a starting settler");
        let here = g.units[&settler].pos;
        // Block every plot but the one underfoot.
        for pos in g.map.tiles.keys().copied().collect::<Vec<_>>() {
            if pos != here {
                std::sync::Arc::make_mut(&mut g.blocked_city_sites).insert(pos);
            }
        }
        let mut ai = AdvancedAi::new();
        ai.enable_settler_never_idles();
        let journal = Journal::recording();
        ai.attach_journal(journal.handle());
        assert_eq!(
            ai.settler_exhaustion_target(&g, 0, settler),
            Some(here),
            "the tile underfoot is the one legal site"
        );
        assert!(g.can_found_city(settler));
        assert!(ai.settler_stranded(&mut g, 0, settler));
        assert_eq!(
            g.player_city_ids(0).len(),
            1,
            "the capital stands where it stood"
        );
        // A second Settler in that city cannot found (spacing) and has no
        // site anywhere: it reports itself stranded rather than holding
        // silently.
        let second = g.spawn_test_unit("settler", 0, here);
        assert!(ai.settler_exhaustion_target(&g, 0, second).is_none());
        assert!(!ai.settler_stranded(&mut g, 0, second));
        let delta = journal.since(0);
        assert!(
            delta
                .thoughts
                .iter()
                .any(|thought| thought.headline.starts_with("Settler is stranded")),
            "the hold is named"
        );
    }
}
