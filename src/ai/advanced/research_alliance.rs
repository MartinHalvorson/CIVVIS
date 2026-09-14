//! Research alliance first: the science an Emperor handicap cannot deny us.
//!
//! ★★★ AT EMPEROR AND ABOVE EVERY RIVAL YIELD IS MULTIPLIED AND OURS IS NOT.
//! `data/difficulties.json` gives the AI seats +16..+32 percent on Science,
//! Culture, Gold, Faith and Production; the ladder's answer so far has been to
//! out-build them, which is exactly the axis the handicap owns. An alliance is
//! not on that axis. What the model pays for one is paid to us at full rate:
//!
//! 1. **Alliance points accrue every turn** (`Game::process_diplomacy`,
//!    `src/game/actions.rs:11034-11112`): `+1.0` a turn, **`+0.25` for an
//!    outgoing route to the partner and `+0.25` for an incoming one**, plus
//!    `0.25` per side for Democracy or Wisselbanken. Level 2 at
//!    [`ALLY_LEVEL_TWO_POINTS`], level 3 at `240.0`.
//! 2. **A level-2 Research Alliance shares tech boosts** — every
//!    `standard_duration(30)` turns `share_research_alliance_boosts`
//!    (`src/game/actions.rs:8042`) hands the partner's cheapest unboosted
//!    tech to whichever side lacks it.
//! 3. **A level-3 Research Alliance adds 10 percent of the ally's whole
//!    science** to ours (`src/game.rs:32182`,
//!    `ALLIANCE_SCIENCE_SHARING_FROM_ALLY`).
//! 4. **A route to an ally is already worth `+2` Science and a flat `45.0`
//!    for the first connection** in `trade_route_destination_value_from`;
//!    nothing there asks whether that connection can still buy a level.
//!
//! # What this gene does, and what it deliberately does not
//!
//! The whole of it runs **through** `propose_strategic_alliance`, the stock
//! alliance desk, so the desk's own exclusions — the victory-denial partner,
//! the `rival_victory_pressure(other).progress < 82` ceiling, the grievance
//! ceiling, an open proposal either way — apply unchanged. With the gene on:
//!
//! 1. **The kind is Research**, whatever the grand strategy names, for as
//!    long as [`Self::research_alliance_lane`] says a Research Alliance is
//!    still worth waiting for: none is held, and level 2 — the first level
//!    that pays — can still be reached before the turn limit. While our own
//!    tree lacks `scientific_theory` the desk **waits** and proposes no other
//!    kind: an empire holds one alliance per partner
//!    (`Player.alliances: BTreeMap<usize, AllianceState>`), so an economic or
//!    cultural alliance with the best science partner is the Research
//!    Alliance that can never be seated. Once level 2 is out of reach the
//!    stock kind resumes.
//! 2. **The twelve-turn cadence is not waited for.** The stock desk asks on
//!    `g.turn % 12 == pid % 12`; a legal research partner known on the wrong
//!    turn is asked eleven turns late, and every one of those turns is an
//!    alliance point not accrued. A partner who has refused is not asked
//!    again for [`ALLY_RETRY_TURNS`] turns.
//! 3. **The ranking gains a science term**: [`ALLY_SCIENCE_WEIGHT`] times the
//!    partner's science as a share of ours, the number level 3 pays 10
//!    percent of. The stock term — the count of techs they hold and we lack,
//!    times four — stays; it prices the level-2 boosts.
//! 4. **A culture threat is never a partner.** The proposal bundles passage,
//!    and passage is +25 percent tourism against us
//!    (`Game::international_tourism_multiplier`).
//! 5. **The first route to the research ally carries
//!    [`ALLY_ROUTE_PREMIUM`]** while the alliance is below level 3, capped to
//!    [`ALLY_ROUTE_PREMIUM_OPENING_CAP`] inside the opening band.
//!
//! ⚠⚠ **No declared friendship is proposed ahead of the alliance.** The
//! engine does not require one (`Game::do_propose_deal`,
//! `src/game/actions.rs:8774-8841`, reads nothing of `friends_until`), and
//! the first version of this gene, which led with a friendship from turn 16,
//! cost **−3.5 pp of score share at |z| 3.3** across three probe versions
//! (`docs/eval/2026-09-09-research-alliance-first.md`). The mechanism, found
//! by stripping `friends_until` from a cloned turn-20 world and replaying the
//! stock controller on both copies: `campaign_target_legal` refuses a friend,
//! so the friendship with the strongest neighbour deleted the opening plan's
//! campaign target (`target=Some(3), city=Some(56)` → `None`) and the whole
//! opening posture — warrior staging, escort, second-city timing — hangs off
//! that target. The befriended seat sat at one city past turn 80 where the
//! stock seat had two at turn 40. Every other part of the first version (the
//! fallback kinds, the route premium, the card) measured as nothing.
//!
//! Counter: `research_alliance:alliances_proposed`.

use super::AdvancedAi;
use crate::game::Game;

/// Weight on the partner's science as a share of ours in the partner
/// ranking. A partner our equal in science adds as much as the stock
/// friendship term (`180.0`) less the stock connection term (`70.0`): enough
/// to prefer a science-heavy stranger to a science-poor friend, not enough to
/// prefer one over a friend already connected by a route.
pub(crate) const ALLY_SCIENCE_WEIGHT: f64 = 110.0;

/// A partner asked for an alliance is not asked again for this many turns.
/// Matches `COALITION_REASK_TURNS`: the cadence this gene bypasses is
/// twelve, and a refusal answered the next turn would be answered the same.
pub(crate) const ALLY_RETRY_TURNS: u32 = 10;

/// Alliance points at which level 2 — the first level that pays anything on
/// the Research tier — is reached (`Game::process_diplomacy`,
/// `src/game/actions.rs:11034-11112`).
pub(crate) const ALLY_LEVEL_TWO_POINTS: f64 = 80.0;

/// The most alliance points a turn can bring with one route each way:
/// `1.0 + 0.25 + 0.25`. Democracy and Wisselbanken add a quarter each but are
/// not counted on: the question is whether level 2 is reachable at all.
pub(crate) const ALLY_POINTS_PER_TURN_ROUTED: f64 = 1.5;

/// Extra route score for the first route to the research ally while the
/// alliance is still below [`ALLY_TOP_LEVEL`]. Sits on top of the stock
/// alliance premium in `trade_route_destination_value_from` (the ally's `+2`
/// Science and the flat `45.0` for a first connection), which is blind to
/// whether that connection can still buy a level.
pub(crate) const ALLY_ROUTE_PREMIUM: f64 = 30.0;

/// Inside the opening band the premium is capped to this. Every live win
/// came from four to six cities at turn 60 (`docs/gene_screens/`, the
/// opening-band study), and those cities are grown by internal food and
/// production routes; an alliance point is not worth displacing one.
pub(crate) const ALLY_ROUTE_PREMIUM_OPENING_CAP: f64 = 10.0;

/// An empire with fewer cities than this is inside the opening band.
pub(crate) const ALLY_ROUTE_OPENING_BAND_CITIES: usize = 6;

/// The alliance level at which the Research tier shares 10 percent of the
/// ally's science (`src/game.rs:32182`). Points past it buy nothing, so the
/// route premium stops here.
pub(crate) const ALLY_TOP_LEVEL: i32 = 3;

/// The kind of alliance this desk asks for.
pub(crate) const ALLY_RESEARCH_KIND: &str = "research";

/// What the gene tells the stock alliance desk to do this turn.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ResearchAllianceLane {
    /// The gene is off, a Research Alliance already stands, or level 2 is out
    /// of reach before the turn limit: the desk runs exactly as shipped.
    Stock,
    /// A Research Alliance is worth waiting for and not yet legal on our own
    /// tree: propose no other kind, so the partner's slot stays open for it.
    Wait,
    /// Propose a Research Alliance, this turn, cadence or not.
    Research,
}

/// A partner's science measured against our own, as the ranking uses it.
/// Our own zero makes any partner infinitely better, which is right: an
/// empire with no science has nothing to protect and everything to gain —
/// and a finite score is still needed, so the ratio is clamped to the weight.
pub(crate) fn science_share(partner_science: f64, own_science: f64) -> f64 {
    if own_science <= f64::EPSILON {
        return if partner_science > 0.0 {
            ALLY_SCIENCE_WEIGHT
        } else {
            0.0
        };
    }
    (partner_science / own_science).min(ALLY_SCIENCE_WEIGHT)
}

/// The route premium for one destination, given the alliance level, whether a
/// route to that ally already exists, and the city count. Pure, so the cap and
/// the two stopping conditions are testable without a board.
pub(crate) fn route_premium(level: i32, already_connected: bool, cities: usize) -> f64 {
    if already_connected || level >= ALLY_TOP_LEVEL {
        return 0.0;
    }
    if cities < ALLY_ROUTE_OPENING_BAND_CITIES {
        ALLY_ROUTE_PREMIUM.min(ALLY_ROUTE_PREMIUM_OPENING_CAP)
    } else {
        ALLY_ROUTE_PREMIUM
    }
}

/// Can an alliance seated on `turn` still reach level 2 before the turn
/// limit, with one route each way? Pure.
pub(crate) fn level_two_reachable(turn: u32, max_turns: u32) -> bool {
    let turns_needed = (ALLY_LEVEL_TWO_POINTS / ALLY_POINTS_PER_TURN_ROUTED).ceil() as u32;
    max_turns.saturating_sub(turn) >= turns_needed
}

impl AdvancedAi {
    /// Total science of an empire's cities. The same sum
    /// `ALLIANCE_SCIENCE_SHARING_FROM_ALLY` takes 10 percent of, so the
    /// ranking is measured in the units the alliance actually pays in.
    pub(crate) fn research_alliance_science(g: &Game, pid: usize) -> f64 {
        g.player_city_ids(pid)
            .into_iter()
            .map(|city| g.city_yields(city).science)
            .sum()
    }

    /// Do we already hold the alliance this gene exists to get?
    fn research_alliance_held(g: &Game, pid: usize) -> bool {
        g.players[pid]
            .alliances
            .values()
            .any(|alliance| alliance.ends > g.turn && alliance.kind == ALLY_RESEARCH_KIND)
    }

    /// What the stock alliance desk does this turn under the gene. `Stock`
    /// whenever the gene is off, so the desk is byte-identical off.
    pub(crate) fn research_alliance_lane(&self, g: &Game, pid: usize) -> ResearchAllianceLane {
        if !self.research_alliance_first
            || Self::research_alliance_held(g, pid)
            || !level_two_reachable(g.turn, g.max_turns)
        {
            return ResearchAllianceLane::Stock;
        }
        if g.tree_effect(pid, "research_agreements") <= 0.0 {
            return ResearchAllianceLane::Wait;
        }
        ResearchAllianceLane::Research
    }

    /// Is this partner barred from the research desk: a culture threat, or
    /// inside the cool-down on our last ask of them? Never barred off.
    pub(crate) fn research_alliance_barred(&self, g: &Game, pid: usize, partner: usize) -> bool {
        if !self.research_alliance_first {
            return false;
        }
        let cooling = self
            .research_alliance_asked
            .get(&partner)
            .is_some_and(|asked| g.turn < asked.saturating_add(ALLY_RETRY_TURNS));
        cooling || self.culture_trade_threats(g, pid).contains(&partner)
    }

    /// The science term of the partner ranking: the partner's science as a
    /// share of ours, weighted. Zero off, and zero for any kind but Research.
    pub(crate) fn research_alliance_science_term(
        &self,
        g: &Game,
        pid: usize,
        partner: usize,
        kind: &str,
    ) -> f64 {
        if !self.research_alliance_first || kind != ALLY_RESEARCH_KIND {
            return 0.0;
        }
        ALLY_SCIENCE_WEIGHT
            * science_share(
                Self::research_alliance_science(g, partner),
                Self::research_alliance_science(g, pid),
            )
    }

    /// A Research Alliance was just proposed under the gene: start the
    /// partner's cool-down and count it. A no-op off.
    pub(crate) fn research_alliance_note_asked(
        &mut self,
        g: &mut Game,
        pid: usize,
        partner: usize,
        kind: &str,
    ) {
        if !self.research_alliance_first || kind != ALLY_RESEARCH_KIND {
            return;
        }
        self.research_alliance_asked.insert(partner, g.turn);
        *g.players[pid]
            .counters
            .entry("research_alliance:alliances_proposed".to_string())
            .or_insert(0) += 1;
    }

    /// The route premium for one destination owner. Zero while the gene is
    /// off, so the route valuation is byte-identical.
    pub(crate) fn research_alliance_route_premium(
        &self,
        g: &Game,
        pid: usize,
        owner: usize,
    ) -> f64 {
        if !self.research_alliance_first {
            return 0.0;
        }
        let Some(alliance) = g
            .alliance_with(pid, owner)
            .filter(|alliance| alliance.kind == ALLY_RESEARCH_KIND)
        else {
            return 0.0;
        };
        let already_connected = g.routes.iter().any(|route| {
            route.owner == pid
                && route.ends > g.turn
                && g.cities
                    .get(&route.dest)
                    .is_some_and(|destination| destination.owner == owner)
        });
        route_premium(
            alliance.level,
            already_connected,
            g.player_city_ids(pid).len(),
        )
    }
}

#[cfg(test)]
mod tests;
