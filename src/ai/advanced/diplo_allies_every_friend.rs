//! `diplo-seat-allies-every-friend`: a Diplomatic seat asks every declared
//! friend for an alliance, a different kind each.
//!
//! ★★ FAVOR IS WHAT THE VOTE IS BOUGHT WITH, AND AN ALLIANCE IS ONE FAVOR A
//! TURN. Six live Deity Rome Diplomatic seats of 2026-11-13 ended on 11, 16,
//! 17, 19, 16 and 17 of twenty points. Their Favor income was 3 a turn at
//! turn 75, 4.5 at 100, 6.5 at 125 and 8 at 150 (medians; Government 2-3,
//! Suzerain 1-4, Embassies 1-4), a claim on the Diplomatic Victory resolution
//! costs 144 Favor for nine votes and 312 for thirteen, and over the day's 20
//! sessions with no gang on us the rivals' largest block was under nine votes
//! in 11, under eleven in 16 and under thirteen in all 20. The bank, not the
//! will, sized every claim that lost.
//!
//! The same seats held **1.5 declared friends a turn on average and 0.04
//! allies**. civvis-20261113T182344Z (16 points at the end) had three
//! declared friends from turn 100 and four from turn 139, to the last turn,
//! 187; it asked one of them for one alliance, once (turn 119, Research,
//! refused), and "+N from Alliances" never appeared in its Favor sources.
//! Every alliance is +1 Favor a turn from its first level ("+1 from
//! Alliances" beside one ally, 195128Z turn 150), forbids the ally's war on us
//! and carries the Defensive Pact (see `military_alliance_shield.rs`).
//!
//! Why the desks ask so little:
//!
//! - `propose_strategic_alliance` asks **one** partner for **one** kind (the
//!   lane's: Research under `research-alliance-first`, Economic on a
//!   Diplomacy plan after that) and returns once we hold that kind.
//! - It refuses a science threat any alliance (`science-threat-denial`) and
//!   any rival at 82 percent of a victory. At Deity nearly every rival
//!   out-researches us: on 182344Z the three friends with 300-350 Science to
//!   our 200 were never candidates, and the ask went to the weakest friend
//!   and then to an Unfriendly rival.
//! - `military-alliance-shield-2` waits for the lane's own alliance
//!   (`shield_lane_alliance_waits`), which on this seat never came.
//!
//! What a declared friend answers (asks that reached a rival in the games of
//! 2026-11-12 and 11-13 still on disk, all lanes): Research 9 accepted of 13,
//! Economic 29 of 53, Cultural 8 of 21, Religious 2 of 6, Military 8 of 104.
//!
//! What the gene does, all of it inert while the flag is off or the seat's
//! assigned victory is not Diplomacy. Once Civil Service is ours, on a turn
//! no other desk has asked for an alliance, one **declared friend** we hold
//! no alliance with is asked for the first of [`DIPLO_ALLY_KINDS`] that is
//! free on both sides and that this partner was not asked for lately:
//!
//! - **Declared friends only.** The host makes an alliance only with one
//!   (`DIPLO_STATE_DECLARED_FRIEND`, read from
//!   `Player::observed_diplomatic_state`; on a native board, the friendship
//!   itself), and a friend is already a rival we may not attack, so the
//!   alliance closes no war the friendship had left open. No friendship is
//!   proposed here: `research-alliance-first` measured what befriending the
//!   strongest neighbour costs an opening.
//! - **Research is not asked of a science threat**: the level-two boosts and
//!   the level-three share are the yield it is winning with. Every other
//!   kind is. On this lane the answer to a space race is the twentieth point,
//!   not a war on a friend, and the plan's own denial target
//!   (`denied_partner`) is still never asked.
//! - **Never a culture threat**: the agent refuses a culture racer
//!   (`CivvisAllianceCultureRacer`), and the ask would only be remembered.
//! - **One ask a turn, and none on a turn another desk asked**: the live
//!   host keeps a batch's first alliance order and holds its sales and
//!   purchases beside it (`hold_deals_beside_an_alliance`).
//! - **Its own memory.** A partner is asked again
//!   [`DIPLO_ALLY_PARTNER_TURNS`] after an ask that did not ally, and not for
//!   the same kind inside `ALLIANCE_REFUSAL_TURNS`; the stock desk's ask to
//!   that partner counts as an ask. The memory of a partner is dropped while
//!   the alliance stands, so the renewal is asked the turn it lapses.
//!
//! Military is last: it is the kind friends refuse.
//!
//! ⚠ No native screen prices this: `player_decision_view` hides rival civics,
//! so no alliance desk proposes on an observed board (see
//! `alliance_remembers_the_refusal.rs` and the 2026-11-04 alliance notes).
//! The proof of fire is a stored live board.
//!
//! Counter: `diplo_allies_every_friend:asked`.

use super::*;

/// The kinds asked, in order: by how often a declared friend accepted each
/// on the live seat (Research 9 of 13, Economic 29 of 53, Cultural 8 of 21,
/// Religious 2 of 6, Military 8 of 104).
pub(super) const DIPLO_ALLY_KINDS: [&str; 5] =
    ["research", "economic", "cultural", "religious", "military"];

/// Turns before this desk asks one partner again, whatever the kind. Three
/// times the agent's own retry floor (`AllianceRetryTurns`, 5): with four
/// friends that all refuse, about one ask every four turns.
pub(super) const DIPLO_ALLY_PARTNER_TURNS: u32 = 15;

/// Grievances against a partner at or over this are a quarrel, not a
/// friendship about to be deepened: the stock desk's own ceiling.
pub(super) const DIPLO_ALLY_GRIEVANCE_CEILING: f64 = 75.0;

impl AdvancedAi {
    /// Whether the gene acts for this seat.
    fn diplo_allies_every_friend_on(&self, g: &Game) -> bool {
        self.diplo_seat_allies_every_friend
            && !self.lane_lost
            && self.active_victory_target(g) == Some(VictoryTarget::Diplomacy)
    }

    /// Whether `other` is a declared friend of `pid`: the host's own reading
    /// of the leader's standing where the board carries one, the friendship
    /// itself on a native board.
    pub(super) fn diplo_ally_declared_friend(g: &Game, pid: usize, other: usize) -> bool {
        match g.players[other].observed_diplomatic_state.get(&pid) {
            Some(state) => state == "DECLARED_FRIEND",
            None => g.are_friends(pid, other),
        }
    }

    /// The last turn any desk asked `partner` for an alliance: this desk's
    /// own asks and the shared memory the stock desk and the coalition write.
    fn diplo_ally_last_ask(&self, partner: usize) -> Option<u32> {
        self.diplo_ally_asks
            .iter()
            .filter(|((who, _), _)| *who == partner)
            .map(|(_, asked)| *asked)
            .chain(
                self.alliance_ask_turns
                    .get(&partner)
                    .map(|(asked, _)| *asked),
            )
            .max()
    }

    /// Whether `kind` was asked of `partner` inside the refusal bar: by this
    /// desk, or, for Research, by `research-alliance-first`.
    fn diplo_ally_kind_barred(&self, g: &Game, partner: usize, kind: &str) -> bool {
        let bar = super::alliance_remembers_the_refusal::ALLIANCE_REFUSAL_TURNS;
        let own = self
            .diplo_ally_asks
            .iter()
            .find(|((who, asked_kind), _)| *who == partner && *asked_kind == kind)
            .map(|(_, asked)| *asked);
        let research = if kind == "research" {
            self.research_alliance_asked.get(&partner).copied()
        } else {
            None
        };
        own.into_iter()
            .chain(research)
            .any(|asked| g.turn < asked.saturating_add(bar))
    }

    /// The declared friend this desk would ask this turn, and the kind: the
    /// partner whose first open kind comes earliest in [`DIPLO_ALLY_KINDS`],
    /// the lower seat on a tie. `None` with the gene off, off the Diplomatic
    /// lane, before Civil Service, or with nobody to ask.
    pub(super) fn diplo_ally_candidate(
        &self,
        g: &Game,
        pid: usize,
        denied_partner: Option<usize>,
    ) -> Option<(usize, &'static str)> {
        if !self.diplo_allies_every_friend_on(g)
            || !g.players[pid]
                .civics
                .contains(&crate::name!("civil_service"))
        {
            return None;
        }
        let turn = g.turn;
        let kind_taken = |who: usize, kind: &str| {
            g.players[who]
                .alliances
                .values()
                .any(|alliance| alliance.ends > turn && alliance.kind == kind)
        };
        let research_legal = |who: usize| g.tree_effect(who, "research_agreements") > 0.0;
        let science_threats = self.science_threat_seats(g, pid);
        let culture_threats = self.culture_trade_threats(g, pid);
        let denounced = |holder: usize, against: usize| {
            g.players[holder]
                .denounced_until
                .get(&against)
                .is_some_and(|until| *until > turn)
        };
        let mut best: Option<(usize, usize, &'static str)> = None;
        for other in g.players.iter() {
            let id = other.id;
            if id == pid
                || !other.alive
                || other.is_minor
                || other.is_barbarian
                || !g.has_met(pid, id)
                || g.is_at_war(pid, id)
                || Some(id) == denied_partner
                || !other.civics.contains(&crate::name!("civil_service"))
                || g.alliance_with(pid, id).is_some()
                || !Self::diplo_ally_declared_friend(g, pid, id)
                || denounced(pid, id)
                || denounced(id, pid)
                || g.players[pid].grievances.get(&id).copied().unwrap_or(0.0)
                    >= DIPLO_ALLY_GRIEVANCE_CEILING
                || culture_threats.contains(&id)
                || g.pending_deals.iter().any(|deal| {
                    deal.expires >= turn
                        && ((deal.from == pid && deal.to == id)
                            || (deal.from == id && deal.to == pid))
                })
                || self
                    .diplo_ally_last_ask(id)
                    .is_some_and(|asked| turn < asked.saturating_add(DIPLO_ALLY_PARTNER_TURNS))
            {
                continue;
            }
            let open = DIPLO_ALLY_KINDS.iter().copied().position(|kind| {
                !kind_taken(pid, kind)
                    && !kind_taken(id, kind)
                    && !self.diplo_ally_kind_barred(g, id, kind)
                    && (kind != "research"
                        || (research_legal(pid)
                            && research_legal(id)
                            && !self.science_denial_refuses_alliance(&science_threats, id)))
            });
            let Some(rank) = open else {
                continue;
            };
            if best.is_none_or(|(top, _, _)| rank < top) {
                best = Some((rank, id, DIPLO_ALLY_KINDS[rank]));
            }
        }
        best.map(|(_, partner, kind)| (partner, kind))
    }

    /// `diplo-seat-allies-every-friend`: ask one declared friend for an
    /// alliance. Runs after the other alliance desks and stands down on a
    /// turn any of them asked. Returns whether it asked.
    pub(super) fn diplo_allies_every_friend_step(
        &mut self,
        g: &mut Game,
        pid: usize,
        denied_partner: Option<usize>,
    ) -> bool {
        if !self.diplo_allies_every_friend_on(g) {
            return false;
        }
        let turn = g.turn;
        // An alliance that stands needs no memory: its renewal is asked the
        // turn it lapses.
        self.diplo_ally_asks
            .retain(|(partner, _), _| g.alliance_with(pid, *partner).is_none());
        // One alliance ask a turn across every desk and every decision of the
        // turn: the live host keeps a batch's first alliance order only, yet
        // a dropped ask would be remembered as made.
        if g.pending_deals
            .iter()
            .any(|deal| deal.from == pid && deal.alliance.is_some() && deal.expires >= turn)
            || self.diplo_ally_asks.values().any(|asked| *asked == turn)
            || self
                .alliance_ask_turns
                .values()
                .any(|(asked, _)| *asked == turn)
            || self
                .military_alliance_shield_asks
                .values()
                .any(|(asked, _)| *asked == turn)
        {
            return false;
        }
        let Some((partner, kind)) = self.diplo_ally_candidate(g, pid, denied_partner) else {
            return false;
        };
        let proposal = Action::ProposeDeal {
            player: partner,
            give_gold: 0.0,
            request_gold: 0.0,
            // Passage is not bundled: it is +25 percent tourism against us,
            // and the host's alliance order carries the kind alone.
            open_borders: false,
            friendship: true,
            peace: false,
            alliance: Some(kind.to_string()),
        };
        if g.apply(pid, &proposal).is_err() {
            return false;
        }
        self.diplo_ally_asks.insert((partner, kind), turn);
        *g.players[pid]
            .counters
            .entry("diplo_allies_every_friend:asked".to_string())
            .or_insert(0) += 1;
        think!(self.journal(), Diplomacy, Decision,
               "Asking {} for a {kind} alliance", g.players[partner].civ;
               "they are a declared friend we hold no alliance with: each alliance is one \
                more Favor a turn for the vote, and an ally cannot declare on us");
        true
    }
}

#[cfg(test)]
mod tests;
