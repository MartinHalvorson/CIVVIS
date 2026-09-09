//! Research alliance first: the science an Emperor handicap cannot deny us.
//!
//! ★★★ AT EMPEROR AND ABOVE EVERY RIVAL YIELD IS MULTIPLIED AND OURS IS NOT.
//! `data/difficulties.json` gives the AI seats +16..+32 percent on Science,
//! Culture, Gold, Faith and Production; the ladder's answer so far has been to
//! out-build them, which is exactly the axis the handicap owns. An alliance is
//! not on that axis. What the model pays for one is paid to us at full rate:
//!
//! 1. **Alliance points accrue every turn** (`Game::end_turn_diplomacy`,
//!    `src/game/actions.rs:11041-11090`): `+1.0` a turn, **`+0.25` for an
//!    outgoing route to the partner and `+0.25` for an incoming one**, plus
//!    `0.25` per side for Democracy or Wisselbanken. Level 2 at `80.0`
//!    points, level 3 at `240.0`.
//! 2. **A level-2 Research Alliance shares tech boosts** — every
//!    `standard_duration(30)` turns `share_research_alliance_boosts` hands the
//!    partner's cheapest unboosted tech to whichever side lacks it
//!    (`Game::research_alliance_boost_candidate`).
//! 3. **A level-3 Research Alliance adds 10 percent of the ally's whole
//!    science** to ours (`src/game.rs:32182`,
//!    `ALLIANCE_SCIENCE_SHARING_FROM_ALLY`, the Cultural tier's twin).
//! 4. **A route to an ally is already worth `+2` Science** in
//!    `trade_route_destination_value_from`, and `trade_confederation` /
//!    `market_economy` add `+1` / `+2` Science to every international route
//!    — `data/policies.json`'s `international_trade_science`, read by
//!    `src/game/city.rs:4082` and owned by no other card in the tree, so no
//!    domestic route earns it.
//!
//! Nothing in the genome prices any of this. `propose_strategic_alliance`
//! picks a kind from the grand strategy on a twelve-turn cadence
//! (`g.turn % 12 != pid % 12`), so a legal research partner known at turn 61
//! is first asked at turn 72 at the earliest, and its ranking term for a
//! research partner is the count of techs the partner holds and we do not —
//! a measure of what we can copy once, not of the science that will still be
//! flowing at level 3. `coalition.rs` proposes alliances early, but only
//! military ones and only in front of a war.
//!
//! What the gene does, all of it inert while the flag is off:
//!
//! 1. **A partner ranking.** Among met, living majors we are not at war with
//!    and who are neither the current culture threat nor the current science
//!    threat, rank by the partner's science measured against our own
//!    ([`ALLY_MIN_SCIENCE_SHARE`] keeps a trivial partner out) plus the
//!    feasibility of actually getting the alliance: a declared friendship
//!    already in place, `research_agreements` on both trees, no denouncement
//!    in either direction, grievances under [`ALLY_GRIEVANCE_CEILING`].
//! 2. **A sequence, one proposal a turn.** No friendship yet: propose the
//!    declared friendship. Friendship in place and a Research Alliance legal
//!    on both sides: propose it. Only another kind free: take the one whose
//!    *model* yield is closest to science ([`ALLY_FALLBACK_KINDS`]) and
//!    journal which. A partner asked is not asked again for
//!    [`ALLY_RETRY_TURNS`] turns. The stock cadence is not waited for.
//!
//!    The objective is **one** Research Alliance: once it stands the desk
//!    stops proposing entirely and only the routes and the card keep
//!    working. Until then the ranking simply names the best partner who is
//!    not already pending or cooling down, so a turn spent waiting for one
//!    answer is spent asking the next partner rather than idling — a
//!    friendship costs nothing to hold and is the prerequisite either of
//!    them would need.
//!
//!    ⚠ The engine does **not** require a declared friendship for an
//!    alliance: `Game::do_propose_deal` (`src/game/actions.rs:8818-8841`)
//!    asks only for a valid kind, `civil_service` on both trees,
//!    `research_agreements` on both for `"research"`, the kind free on both
//!    sides, peace, and no denouncement. The friendship gate is the
//!    controllers' — `src/ai.rs:8511` makes the Basic controller wait for
//!    `are_friends` — and the operator's design asks for the same sequence
//!    here. It costs one proposal turn; whether a bundled
//!    friendship-and-alliance proposal lands more often than the two-step is
//!    an open question this gene does not answer.
//! 3. **Routes.** While an alliance is in place and still below the top
//!    level, the first route to that ally's cities carries
//!    [`ALLY_ROUTE_PREMIUM`] — the real accrual above is what it buys, so the
//!    premium stops at level 3, where a further point buys nothing, and at
//!    the second route, where the `+0.25` is already collected. Inside the
//!    opening band it is capped to [`ALLY_ROUTE_PREMIUM_OPENING_CAP`] so an
//!    internal food or production route is not displaced.
//! 4. **A card.** With such an alliance active, the best offered
//!    science-per-international-route card is wanted ahead of the plan's
//!    ordinary deck, as `culture_defense_cards` does for tourism defence.
//! 5. **A guard.** A culture or science threat is never proposed to, never
//!    carries the route premium, and never justifies the card. An alliance
//!    already standing with a partner who becomes a threat is **not broken**
//!    — breaking one costs grievance and the science is still real; the
//!    objective is simply dropped.
//!
//! Counters: `research_alliance:friendships_proposed`,
//! `research_alliance:alliances_proposed`.

use std::collections::BTreeMap;

use super::{AdvancedAi, GrandStrategy};
use crate::game::{Action, Game};
use crate::think;

/// A partner whose science is below this share of our own is not worth an
/// alliance slot: at level 3 the share is 10 percent of the partner's whole
/// output, so a partner at 0.6 of us is worth 6 percent of our own science
/// and one at 0.2 is worth 2 — inside the noise of a single Library.
pub(crate) const ALLY_MIN_SCIENCE_SHARE: f64 = 0.6;

/// Weight on the partner's science share in the ranking. One whole unit of
/// share (a partner our equal in science) outranks the
/// [`ALLY_GRIEVANCE_CEILING`] of grievance that would exclude them anyway.
pub(crate) const ALLY_SCIENCE_WEIGHT: f64 = 100.0;

/// A declared friendship already in place is the single largest feasibility
/// term: it is the one prerequisite an alliance cannot be proposed without,
/// and it takes a whole extra proposal turn to obtain. The stock ranking in
/// `propose_strategic_alliance` prices it at exactly this.
pub(crate) const ALLY_FRIENDSHIP_READY: f64 = 180.0;

/// Both trees carrying `research_agreements` is what makes the *Research*
/// alliance — the only kind that shares science — legal at all.
pub(crate) const ALLY_RESEARCH_LEGAL: f64 = 60.0;

/// Grievance above this in either direction and there is no partner here.
/// The ceiling `propose_strategic_alliance` already applies.
pub(crate) const ALLY_GRIEVANCE_CEILING: f64 = 75.0;

/// A partner asked for friendship or an alliance is not asked again for this
/// many turns. Matches `COALITION_REASK_TURNS`: one full round of every
/// empire answering its incoming deals, several times over.
pub(crate) const ALLY_RETRY_TURNS: u32 = 10;

/// A rival whose leading victory lane is Science and whose progress in it has
/// reached this percentage is a science threat, not a partner. The bar is
/// `CULTURE_THREAT_PRESSURE_EARLY`'s, deliberately: the same 30 percent of a
/// lane that arms the culture defence.
pub(crate) const ALLY_SCIENCE_THREAT_PROGRESS: i32 = 30;

/// Extra route score for the first route to an ally still climbing towards
/// the science-sharing level. Sits on top of the stock alliance premium in
/// `trade_route_destination_value_from` (a research ally's `+2` Science and
/// the flat `45.0` for a first connection), which is blind to whether that
/// connection can still buy a level.
pub(crate) const ALLY_ROUTE_PREMIUM: f64 = 30.0;

/// Inside the opening band the premium is capped to this. Every live win came
/// from four to six cities at turn 60
/// (`docs/gene_screens/`, the opening-band study), and those cities are grown
/// and built by internal food and production routes; an alliance point is not
/// worth displacing one.
pub(crate) const ALLY_ROUTE_PREMIUM_OPENING_CAP: f64 = 10.0;

/// An empire with fewer cities than this is inside the opening band.
pub(crate) const ALLY_ROUTE_OPENING_BAND_CITIES: usize = 6;

/// The alliance level at which the Research tier begins sharing 10 percent of
/// the ally's science (`src/game.rs:32182`). Points past it buy nothing, so
/// the route premium stops here.
pub(crate) const ALLY_TOP_LEVEL: i32 = 3;

/// When a Research Alliance is not legal, the kinds in order of how close
/// their *model* yield is to science. `cultural` is the Research tier's exact
/// twin — the same level 3, the same 10 percent of the ally's output
/// (`src/game.rs:32171`) — and its level-2 route adds a Great Person point a
/// district, Great Scientists included (`src/game.rs:15595`). `economic` pays
/// the largest route yield (`+4` Gold) and gold buys Campus buildings.
/// `religious` pays Faith, which buys none. `military` pays no yield at all.
pub(crate) const ALLY_FALLBACK_KINDS: [&str; 4] =
    ["cultural", "economic", "religious", "military"];

/// Science per international trade route, by card, best first. Both are
/// ordinary economic-slot cards; the deck machinery drops whichever is not
/// offered. `wisselbanken` is deliberately absent: its `+2` Food and `+2`
/// Production on alliance routes is not science, and its `0.25` alliance
/// points a turn is bought with a diplomatic slot the congress wants.
pub(crate) const ALLY_ROUTE_CARDS: [&str; 2] = ["market_economy", "trade_confederation"];

/// The partners this empire has already asked, so a refusal is not re-asked
/// on the next turn. Present only while the gene is on.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct ResearchAllianceDesk {
    /// Partners asked for a friendship or an alliance, by the turn last asked.
    pub(crate) asked: BTreeMap<usize, u32>,
}

/// A partner's science measured against our own, as the ranking uses it.
/// Our own zero makes any partner infinitely better, which is right: an
/// empire with no science has nothing to protect and everything to gain.
pub(crate) fn science_share(partner_science: f64, own_science: f64) -> f64 {
    if own_science <= f64::EPSILON {
        return if partner_science > 0.0 {
            f64::INFINITY
        } else {
            0.0
        };
    }
    partner_science / own_science
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

impl AdvancedAi {
    /// A rival whose leading lane is Science and who has reached
    /// [`ALLY_SCIENCE_THREAT_PROGRESS`] percent of it. The science twin of
    /// `culture_trade_threats`, built from the same public victory-screen
    /// signal the denial layer reads (`rival_victory_pressure`).
    pub(crate) fn research_alliance_science_threat(&self, g: &Game, rival: usize) -> bool {
        if !self.victory_planning || !g.victory_conditions.science {
            return false;
        }
        let focus = self.rival_victory_pressure(g, rival);
        focus.strategy == GrandStrategy::Science
            && focus.progress >= ALLY_SCIENCE_THREAT_PROGRESS
    }

    /// The guard: a rival racing us to a culture or a science finish is never
    /// a partner, never carries the route premium, and never justifies the
    /// card. An alliance already standing with them is left alone.
    pub(crate) fn research_alliance_excluded(&self, g: &Game, pid: usize, rival: usize) -> bool {
        self.culture_trade_threats(g, pid).contains(&rival)
            || self.research_alliance_science_threat(g, rival)
    }

    /// Total science of an empire's cities. The same sum
    /// `ALLIANCE_SCIENCE_SHARING_FROM_ALLY` takes 10 percent of, so the
    /// ranking is measured in the units the alliance actually pays in.
    pub(crate) fn research_alliance_science(g: &Game, pid: usize) -> f64 {
        g.player_city_ids(pid)
            .into_iter()
            .map(|city| g.city_yields(city).science)
            .sum()
    }

    /// Is a typed alliance of this kind free on both sides? An empire holds
    /// at most one alliance of each kind.
    fn research_alliance_kind_free(g: &Game, pid: usize, partner: usize, kind: &str) -> bool {
        let taken = |who: usize| {
            g.players[who]
                .alliances
                .values()
                .any(|alliance| alliance.ends > g.turn && alliance.kind == kind)
        };
        !taken(pid) && !taken(partner)
    }

    /// Is a Research Alliance legal between these two — the kind free on both
    /// sides and `research_agreements` on both trees? The two conditions
    /// `propose_strategic_alliance` checks before it will name `"research"`.
    fn research_alliance_research_legal(g: &Game, pid: usize, partner: usize) -> bool {
        Self::research_alliance_kind_free(g, pid, partner, "research")
            && g.tree_effect(pid, "research_agreements") > 0.0
            && g.tree_effect(partner, "research_agreements") > 0.0
    }

    /// The ranking term for one candidate, or `None` when they are no
    /// candidate at all. Every exclusion is here, so the caller is a `max_by`.
    fn research_alliance_partner_score(&self, g: &Game, pid: usize, other: usize) -> Option<f64> {
        let turn = g.turn;
        let player = &g.players[other];
        if other == pid
            || !player.alive
            || player.is_minor
            || player.is_barbarian
            || g.same_team(pid, other)
            || !g.has_met(pid, other)
            || g.is_at_war(pid, other)
            || g.alliance_with(pid, other).is_some()
        {
            return None;
        }
        // Civil Service is the engine's gate on any typed alliance; without it
        // on both trees a proposal is refused before it is valued.
        let civil_service = crate::name!("civil_service");
        if !g.players[pid].civics.contains(&civil_service) || !player.civics.contains(&civil_service)
        {
            return None;
        }
        let denounced = |holder: usize, against: usize| {
            g.players[holder]
                .denounced_until
                .get(&against)
                .is_some_and(|until| *until > turn)
        };
        let grievance = |holder: usize, against: usize| {
            g.players[holder]
                .grievances
                .get(&against)
                .copied()
                .unwrap_or(0.0)
        };
        if denounced(pid, other)
            || denounced(other, pid)
            || grievance(pid, other) >= ALLY_GRIEVANCE_CEILING
            || grievance(other, pid) >= ALLY_GRIEVANCE_CEILING
        {
            return None;
        }
        if self.research_alliance_excluded(g, pid, other) {
            return None;
        }
        let share = science_share(
            Self::research_alliance_science(g, other),
            Self::research_alliance_science(g, pid),
        );
        if share < ALLY_MIN_SCIENCE_SHARE {
            return None;
        }
        // An infinite share (our own science is zero) still has to produce a
        // finite, comparable score.
        let science = ALLY_SCIENCE_WEIGHT * share.min(ALLY_SCIENCE_WEIGHT);
        let friendship = if g.are_friends(pid, other) {
            ALLY_FRIENDSHIP_READY
        } else {
            0.0
        };
        let research = if Self::research_alliance_research_legal(g, pid, other) {
            ALLY_RESEARCH_LEGAL
        } else {
            0.0
        };
        Some(science + friendship + research - grievance(pid, other))
    }

    /// The best partner this turn, cool-down and open proposals applied.
    pub(crate) fn research_alliance_partner(&self, g: &Game, pid: usize) -> Option<usize> {
        let turn = g.turn;
        let asked_recently = |partner: usize| {
            self.research_alliance
                .as_ref()
                .and_then(|desk| desk.asked.get(&partner).copied())
                .is_some_and(|asked| turn < asked.saturating_add(ALLY_RETRY_TURNS))
        };
        let pending_with = |partner: usize| {
            g.pending_deals.iter().any(|deal| {
                deal.expires >= turn
                    && ((deal.from == pid && deal.to == partner)
                        || (deal.from == partner && deal.to == pid))
            })
        };
        g.players
            .iter()
            .map(|other| other.id)
            .filter(|other| !asked_recently(*other) && !pending_with(*other))
            .filter_map(|other| {
                self.research_alliance_partner_score(g, pid, other)
                    .map(|score| (score, other))
            })
            .max_by(|left, right| {
                left.0
                    .total_cmp(&right.0)
                    .then_with(|| right.1.cmp(&left.1))
            })
            .map(|(_, other)| other)
    }

    /// The alliance kind to ask this partner for: `"research"` when it is
    /// legal, else the free kind whose model yield is closest to science.
    /// `None` when every kind is taken on one side or the other.
    pub(crate) fn research_alliance_kind(
        &self,
        g: &Game,
        pid: usize,
        partner: usize,
    ) -> Option<&'static str> {
        if Self::research_alliance_research_legal(g, pid, partner) {
            return Some("research");
        }
        ALLY_FALLBACK_KINDS
            .into_iter()
            .find(|kind| Self::research_alliance_kind_free(g, pid, partner, kind))
    }

    /// Do we already hold the alliance this desk exists to get?
    fn research_alliance_held(g: &Game, pid: usize) -> bool {
        g.players[pid]
            .alliances
            .values()
            .any(|alliance| alliance.ends > g.turn && alliance.kind == "research")
    }

    /// One proposal a turn: the declared friendship the alliance needs, or —
    /// once it stands — the alliance itself. Inert while the gene is off.
    pub(crate) fn research_alliance_step(&mut self, g: &mut Game, pid: usize) {
        if !self.research_alliance_first {
            return;
        }
        let turn = g.turn;
        // The objective is dropped for a partner who became a threat: the
        // desk forgets them, so a later thaw is a clean first ask. The
        // alliance itself, if one stands, is deliberately left alone.
        let dropped: Vec<usize> = self
            .research_alliance
            .as_ref()
            .map(|desk| desk.asked.keys().copied().collect())
            .unwrap_or_default();
        let dropped: Vec<usize> = dropped
            .into_iter()
            .filter(|partner| self.research_alliance_excluded(g, pid, *partner))
            .collect();
        if let Some(desk) = self.research_alliance.as_mut() {
            for partner in dropped {
                desk.asked.remove(&partner);
            }
        }
        // The objective is one Research Alliance. Once it stands the desk is
        // done: further proposals would spend friendship and alliance slots
        // on kinds this gene does not want.
        if Self::research_alliance_held(g, pid) {
            return;
        }
        let Some(partner) = self.research_alliance_partner(g, pid) else {
            return;
        };
        let friends = g.are_friends(pid, partner);
        let kind = if friends {
            match self.research_alliance_kind(g, pid, partner) {
                Some(kind) => Some(kind),
                // Friendship stands and every kind is taken: nothing to ask.
                None => return,
            }
        } else {
            None
        };
        let proposal = Action::ProposeDeal {
            player: partner,
            give_gold: 0.0,
            request_gold: 0.0,
            // `no_free_passage`: passage is sold, not bundled.
            open_borders: !self.base.no_free_passage
                && g.players[pid]
                    .civics
                    .contains(&crate::name!("early_empire")),
            friendship: true,
            peace: false,
            alliance: kind.map(str::to_string),
        };
        if g.apply(pid, &proposal).is_err() {
            return;
        }
        self.research_alliance
            .get_or_insert_with(ResearchAllianceDesk::default)
            .asked
            .insert(partner, turn);
        let counter = match kind {
            Some(_) => "research_alliance:alliances_proposed",
            None => "research_alliance:friendships_proposed",
        };
        *g.players[pid]
            .counters
            .entry(counter.to_string())
            .or_insert(0) += 1;
        if !self.journal().wants(crate::reasoning::Level::Decision) {
            return;
        }
        match kind {
            Some("research") => think!(self.journal(), Diplomacy, Decision,
                   "Proposing a Research Alliance to {}", g.players[partner].civ;
                   "their science is worth allying for and the handicap does not touch \
                    a shared tech boost or the level-three science share"),
            Some(kind) => think!(self.journal(), Diplomacy, Decision,
                   "Proposing a {} alliance to {}", kind, g.players[partner].civ;
                   "a Research Alliance is not legal between us; this is the free kind \
                    whose model yield is closest to science"),
            None => think!(self.journal(), Diplomacy, Decision,
                   "Declaring friendship with {}", g.players[partner].civ;
                   "the friendship is the prerequisite of the alliance we want with them"),
        }
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
        let Some(alliance) = g.alliance_with(pid, owner) else {
            return 0.0;
        };
        if self.research_alliance_excluded(g, pid, owner) {
            return 0.0;
        }
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

    /// Is an alliance we want routes and cards for standing right now?
    fn research_alliance_active(&self, g: &Game, pid: usize) -> bool {
        g.players[pid]
            .alliances
            .iter()
            .filter(|(_, alliance)| alliance.ends > g.turn)
            .any(|(partner, _)| !self.research_alliance_excluded(g, pid, *partner))
    }

    /// The card the deck wants ahead of the plan's ordinary portfolio: the
    /// best offered science-per-international-route card, while such an
    /// alliance is active. Empty while the gene is off.
    pub(crate) fn research_alliance_cards(&self, g: &Game, pid: usize) -> Vec<&'static str> {
        if !self.research_alliance_first || !self.research_alliance_active(g, pid) {
            return Vec::new();
        }
        ALLY_ROUTE_CARDS
            .into_iter()
            .find(|card| {
                g.rules
                    .policies
                    .get(*card)
                    .is_some_and(|policy| policy.offered(&g.players[pid].age, g.world_era))
            })
            .into_iter()
            .collect()
    }
}

#[cfg(test)]
mod tests;
