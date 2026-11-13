//! `befriend-the-strongest`: offer a declared friendship — and nothing else —
//! to the strongest major we have met and are at peace with.
//!
//! A friendship forbids a war declaration between the pair while it lasts
//! (`start_war`: "friendship and alliance declarations must expire before
//! war"), and the recipient's `incoming_deal_value` prices a friendship at
//! +40 (+80 on the Diplomacy plan). Yet on the ladder proxy no major formed a
//! single friendship, alliance or defensive pact by turns 60, 100 or 150 —
//! at King or Immortal, against adaptive rivals or against rivals on the
//! Domination lane. The only friendship this controller proposes rides
//! `propose_strategic_alliance`, which waits for Civil Service on both sides
//! and bundles an alliance kind the partner must also want; the stock
//! controller's friendship cadence (`BasicAi::diplomacy`) is not on the
//! advanced turn.
//!
//! With the gene the seat asks every third turn. On the ladder proxy
//! (Immortal, Science lane, every rival on the Domination lane, 32 paired)
//! it held friendships with 2.5 of 3 rivals at turn 100, lost 1.8 cities a
//! game against 3.1, and won 19 games against 4; against the default
//! adaptive rivals it read +1.11 pp of score share. See
//! `docs/eval/2026-09-26-ladder-proxy-immortal.md`.
//!
//! On the live Civilization VI seat a friendship-only deal crosses as a
//! `friendship` order (`civvis_orders`), which the agent mod opens as the
//! shipped `DECLARE_FRIEND` session only toward a rival the host reads as
//! Friendly; Civilization VI's leaders value a declaration from any colder
//! state below zero. The partner is therefore chosen among the rivals the
//! host reads as Friendly toward us (`Player::observed_diplomatic_state`),
//! so a Neutral strongest rival does not hold the offer every turn while a
//! Friendly one is never asked. A native board carries no such reading and
//! chooses exactly as before.

//! **`friendship-keeps-the-peace`** (2026-10-31). Off the Domination lane,
//! a friendship is a war forestalled, not a conquest forgone, and the
//! Domination-era filters (`friendship-only-with-the-stronger`,
//! `friendship-spares-the-prey`) let friendships lapse unrenewed. Across 24
//! live Immortal 6-major seats of 2026-10-31 (every lane but Diplomatic), 34
//! rivals were declared friends at some point, 22 of those friendships
//! lapsed, and 9 of the lapsed friends later declared war on us (t109-t200).
//! civvis-20261031T143921Z (Religion): England was a friend t39-t123, renewed
//! once at t102, read Friendly for one turn after the t123 lapse, Unfriendly
//! from t125, denounced us at t148 and declared war at t175. The live
//! mirror drops `friends_until` the turn a friendship ends, so the gene
//! remembers each expiry while it stands. On a seat aiming at anything but
//! Domination: within [`FRIENDSHIP_RENEW_WINDOW`] turns of a lapse the
//! renewal is asked at once (not on the cadence) and that rival is asked
//! first, and the two Domination filters do not apply. The racer, target and
//! converter guards stay.

use super::*;

/// Every third turn, staggered by seat, so a seat asks a partner who refused
/// again before long without re-proposing every turn.
const FRIENDSHIP_CADENCE: u32 = 3;

/// `friendship-keeps-the-peace`: turns after a declared friendship lapses in
/// which its renewal is asked at once. The host's leaders value a
/// declaration only from Friendly, and live England read Friendly for one
/// turn after its lapse.
pub(super) const FRIENDSHIP_RENEW_WINDOW: u32 = 2;

/// `friendship-ends-at-the-race`: the race progress at which a rival is no
/// longer offered (or re-offered) a friendship. Live Emperor
/// civvis-20261018T075143Z renewed its friendship with Canada at turns 30,
/// 51, 72, 93 and 114; at 150 Canada's Culture race read 54% and "the
/// Conquest counter is barred: we are declared friends".
pub(super) const FRIENDSHIP_RACE_CEILING: i32 = 45;

/// `friendship-refuses-the-racer`: a rival whose culture race is projected to
/// finish within this many turns reads as racing. A declared friendship lasts
/// thirty standard turns and the Conquest counter needs about fifteen more to
/// stage and march, so a race finishing inside both is one the friendship
/// would bar.
pub(super) const FRIENDSHIP_CULTURE_HORIZON: f64 = 45.0;

// `friendship-only-with-the-stronger`: a friendship is offered (and renewed)
// only to a rival whose military power is at least ours. Live Emperor, the
// four games after `befriend-the-strongest` was armed (2026-10-18): of 1,483
// rival-frames under a declared friendship, 1,258 (85%) were with a rival
// weaker than us; civvis-20261018T084359Z kept Greece and France, two cities
// each at turn 120, friends from turn 105 to the end and lost a Technology
// race at 240 holding no rival capital.

// `friendship-spares-the-target`: no friendship is offered (or renewed) to
// the rival the strategic plan's campaign is aimed at. Live Emperor pins g-m
// (25 games, 2026-10-18): 46 of 153 friendship offers went to the current
// campaign target. civvis-20261018T125943Z aimed its campaign at China from
// turn 81 while renewing the friendship at 93 and 114; China stood at 7
// military power against our 579 at turn 100, a declared friend.

/// `friendship-spares-the-prey`: a rival whose military power is below this
/// share of ours is not offered (or re-offered) a friendship. Live Emperor,
/// 2026-10-18 (44 pin-era games): at turn 100, 20 games held a rival under 0.4
/// times our power, and in 11 of them that rival was our declared friend, out
/// of reach (China at 0.01 in civvis-20261018T125943Z, Kongo at 0.10, France at
/// 0.27, Poland at 0.28). Of the 9 we were at war with, 3 lost a city to us.
/// Half, not the parity of `friendship-only-with-the-stronger`: the domination
/// win 090059Z kept India at 0.87 a friend while it took America.
pub(super) const FRIENDSHIP_PREY_RATIO: f64 = 0.5;

// `friendship-spares-the-converter`: no friendship is offered (or renewed) to
// a rival whose founded faith leads the religion race (the most cities
// following it, ours included) or already holds a city of ours. A friendship
// makes the founder an illegal target (`campaign_target_legal`), and every
// religious defence is a war on it: `faith-war-before-the-flip-2`, the
// religious interception and the faith counter. Retained live Immortal games
// to 2026-10-27: 4 of the 14 Religious losses held the faith war on the
// founder converting us as "not a legal target" (040516Z Ethiopia, 184843Z
// Phoenicia, 194829Z Georgia, 033128Z Netherlands). Each friendship was
// offered at turns 42-54, when the founder's faith led the race but no
// spreader stood at our cities and none or one of our cities followed it, so
// `friendship-ends-at-the-race` could not see it: the religion race reads
// 50% only once our own majority has turned.

impl AdvancedAi {
    /// `friendship-spares-the-converter`: whether befriending `rival` would
    /// shield the faith we must fight: Religious Victory is on, `rival`
    /// founded a faith other than ours, and that faith holds a city of ours
    /// or is followed by at least as many cities as any other founded faith
    /// (`Game::cities_following_religion`, the host's World Rankings count).
    pub(super) fn friendship_would_shield_the_converter(
        &self,
        g: &Game,
        pid: usize,
        rival: usize,
    ) -> bool {
        if !g.victory_conditions.religious {
            return false;
        }
        let Some(faith) = g.players.get(rival).and_then(|p| p.religion.as_deref()) else {
            return false;
        };
        if g.players[pid].religion.as_deref() == Some(faith) {
            return false;
        }
        if g.player_city_ids(pid)
            .iter()
            .filter_map(|cid| g.cities.get(cid))
            .any(|city| g.city_religion(city) == Some(faith))
        {
            return true;
        }
        let following = g.cities_following_religion(rival);
        following > 0
            && g.players
                .iter()
                .filter(|other| {
                    other.id != rival
                        && other.alive
                        && !other.is_minor
                        && !other.is_barbarian
                        && other.religion.is_some()
                })
                .all(|other| g.cities_following_religion(other.id) <= following)
    }

    /// `friendship-ends-at-the-race`: whether `rival`'s best race reads
    /// [`FRIENDSHIP_RACE_CEILING`] or more, or the counter reads it urgent.
    /// `friendship-refuses-the-racer`: or its culture race is projected to
    /// finish within [`FRIENDSHIP_CULTURE_HORIZON`]. A friendship outlives
    /// the reading it was offered on: native King Domination seat 30700004
    /// offered Gaul friendship at t105 and t126, under the ceiling, then read
    /// "the Conquest counter is barred: we are declared friends" at 61% and
    /// lost to Gaul's Culture at t169.
    pub(super) fn friend_is_racing(&self, g: &Game, rival: usize) -> bool {
        self.rival_pressure(g, rival).1 >= FRIENDSHIP_RACE_CEILING
            || self.urgent_victory_threat(g, rival)
            || (self.friendship_refuses_the_racer
                && self
                    .projected_culture_finish(g, rival)
                    .is_some_and(|turns| turns <= FRIENDSHIP_CULTURE_HORIZON))
    }

    /// `friendship-keeps-the-peace`: on, and the seat aims at a victory other
    /// than Domination.
    pub(super) fn friendship_keeps_the_peace_on(&self, g: &Game) -> bool {
        self.friendship_keeps_the_peace
            && matches!(self.active_victory_target(g), Some(target) if target != VictoryTarget::Domination)
    }

    /// `friendship-keeps-the-peace`: remember each standing friendship's
    /// expiry (the live mirror drops it when the friendship ends), and name
    /// the rivals whose friendship lapsed within [`FRIENDSHIP_RENEW_WINDOW`]
    /// turns and is not standing now.
    fn friendship_lapses(&self, g: &Game, pid: usize) -> BTreeSet<usize> {
        let mut seen = self.friendship_expiry_seen.borrow_mut();
        for (other, until) in &g.players[pid].friends_until {
            if *until > g.turn {
                seen.insert(*other, *until);
            }
        }
        seen.iter()
            .filter(|(other, until)| {
                !g.are_friends(pid, **other)
                    && **until <= g.turn
                    && g.turn <= **until + FRIENDSHIP_RENEW_WINDOW
            })
            .map(|(other, _)| *other)
            .collect()
    }

    /// Why `other` is not offered a friendship this turn, or `None` when it
    /// is a candidate. [`Self::propose_protective_friendship`] filters on it,
    /// and `friendship-keeps-the-peace` journals the reason a lapsed friend is
    /// not renewed.
    fn friendship_block(
        &self,
        g: &Game,
        pid: usize,
        other: usize,
        keeps_the_peace: bool,
        lapsed: &BTreeSet<usize>,
    ) -> Option<&'static str> {
        let denounced = |by: usize, of: usize| {
            g.players[by]
                .denounced_until
                .get(&of)
                .is_some_and(|until| *until > g.turn)
        };
        let rival = &g.players[other];
        if other == pid
            || !rival.alive
            || rival.is_minor
            || rival.is_barbarian
            || !g.has_met(pid, other)
            || g.same_team(pid, other)
        {
            return Some("not a met rival major");
        }
        if g.is_at_war(pid, other) {
            return Some("we are at war");
        }
        // `score-seat-keeps-friends`: the rival `score-seat-takes-what-pays`
        // names as the campaign's is not befriended. `None` off.
        if self.score_keeps_friends_on(g) && self.score_keeps_friends_spares(g) == Some(other) {
            return Some("score-seat-keeps-friends: the campaign names their city");
        }
        if g.are_friends(pid, other) {
            return Some("already a friend");
        }
        if denounced(pid, other) {
            return Some("we have denounced them");
        }
        if denounced(other, pid) {
            return Some("they have denounced us");
        }
        // A live board carries the host's own reading of this leader's
        // attitude toward us, and the host's leaders value a declared
        // friendship only from FRIENDLY; a colder one would refuse, and the
        // bridge would decline to ask. Absent on a native board, so nothing
        // changes there.
        if let Some(state) = rival
            .observed_diplomatic_state
            .get(&pid)
            .filter(|state| *state != "FRIENDLY")
        {
            // `friendship-keeps-the-peace`: our clock reads a friendship
            // lapsed the turn before the host ends it, so on that turn the
            // host still reads it as standing. Live King
            // civvis-20261101T112906Z journalled Egypt as colder than
            // FRIENDLY at t50, t71, t92 and on, each a turn before the host
            // read FRIENDLY and the renewal landed (t51, t72, t93, ...).
            return Some(if matches!(state.as_str(), "DECLARED_FRIEND" | "ALLIED") {
                "the host still counts the friendship as standing; it ends next turn"
            } else {
                "their attitude toward us is colder than FRIENDLY, and the host befriends only from FRIENDLY"
            });
        }
        // `friendship-ends-at-the-race`: a rival racing to win is left to the
        // counter once the friendship lapses. `friendship-outlasts-the-gated-race`:
        // not while `denial-war-needs-the-lane-edge` would refuse that war.
        if self.friendship_ends_at_the_race
            && self.friend_is_racing(g, other)
            && self
                .gated_race_keeps_the_friend(g, pid, other, lapsed)
                .is_none()
        {
            return Some("friendship-ends-at-the-race: they are racing to win, so they are left to the counter");
        }
        // `friendship-spares-the-prey`: a rival this much weaker is a conquest
        // to make, not a war to forestall.
        if self.friendship_spares_the_prey
            && !keeps_the_peace
            && g.military_power(other) < FRIENDSHIP_PREY_RATIO * g.military_power(pid)
        {
            return Some("friendship-spares-the-prey: a conquest to make");
        }
        // `friendship-spares-the-target`: the rival the campaign is aimed at
        // is a war to open, not one to forbid.
        if self.friendship_spares_the_target
            && self.plan.as_ref().and_then(|plan| plan.target_player) == Some(other)
        {
            return Some("friendship-spares-the-target: the campaign is aimed at them");
        }
        // `first-capital-before-the-walls`: a declared friendship forbids the
        // war the opening is for. `false` with the gene off.
        if self.first_capital_spares_friendship(g, pid, other) {
            return Some("first-capital-before-the-walls: their capital is the opening's target");
        }
        // `friendship-only-with-the-stronger`: a weaker rival is a conquest to
        // make, not a war to forestall.
        if self.friendship_only_with_the_stronger
            && !keeps_the_peace
            && g.military_power(other) < g.military_power(pid)
        {
            return Some("friendship-only-with-the-stronger: weaker than us");
        }
        // `friendship-spares-the-converter`: the founder whose faith we must
        // fight is not made an illegal target.
        // `score-seat-keeps-friends`: on a Score seat only at match point.
        if self.score_friends_spare_the_converter(g, pid, other) {
            return Some("friendship-spares-the-converter: their faith is the one we must fight");
        }
        if g.pending_deals.iter().any(|deal| {
            ((deal.from == pid && deal.to == other) || (deal.from == other && deal.to == pid))
                // `friendship-keeps-the-peace`: our own alliance proposal does
                // not hold back a lapsed friend's renewal. Off the Diplomatic,
                // Culture and Science lanes the live binary drops the alliance
                // order, but the board keeps the deal pending for about ten
                // turns: live Religion seat civvis-20261101T024926Z proposed a
                // Research Alliance to its two friends every ten turns
                // (t129-t170), and the friendships that lapsed at t159 and
                // t162 found a pending deal each and were never renewed. Empty
                // `lapsed` off the gene.
                && !(lapsed.contains(&other) && deal.from == pid && deal.alliance.is_some())
        }) {
            return Some("a deal with them is pending");
        }
        None
    }

    /// Offer a friendship-only deal to the strongest met major at peace with
    /// us that is not already a friend, is not denounced either way, and has
    /// no deal pending with us. Exact no-op with the gene off.
    pub(super) fn propose_protective_friendship(&self, g: &mut Game, pid: usize) {
        // `friendship-keeps-the-peace`: the friendships that lapsed in the
        // last few turns, read before the cadence so the renewal is not
        // missed. Empty with the gene off or on the Domination lane.
        let keeps_the_peace = self.friendship_keeps_the_peace_on(g);
        // `score-seat-keeps-friends`: a Score seat offers every turn, nearest
        // first, with or without `befriend-the-strongest`. See
        // `advanced/score_keeps_friends.rs`.
        let keeps_friends = self.score_keeps_friends_on(g);
        let asks = self.befriend_the_strongest || keeps_friends;
        let lapsed = if keeps_the_peace && asks {
            self.friendship_lapses(g, pid)
        } else {
            BTreeSet::new()
        };
        if !asks
            || (!keeps_friends
                && g.turn % FRIENDSHIP_CADENCE != pid as u32 % FRIENDSHIP_CADENCE
                && lapsed.is_empty())
        {
            return;
        }
        let partner = g
            .players
            .iter()
            .filter(|other| {
                self.friendship_block(g, pid, other.id, keeps_the_peace, &lapsed)
                    .is_none()
            })
            .max_by(|a, b| {
                // `friendship-keeps-the-peace`: a lapsed friend is renewed
                // first. Empty off, so the order is unchanged.
                lapsed
                    .contains(&a.id)
                    .cmp(&lapsed.contains(&b.id))
                    // `score-seat-keeps-friends`: the nearest neighbour next.
                    // Equal (so unchanged) off.
                    .then_with(|| {
                        if keeps_friends {
                            Self::score_friend_distance(g, pid, b.id)
                                .cmp(&Self::score_friend_distance(g, pid, a.id))
                        } else {
                            std::cmp::Ordering::Equal
                        }
                    })
                    .then(g.military_power(a.id).total_cmp(&g.military_power(b.id)))
                    .then(b.id.cmp(&a.id))
            })
            .map(|other| other.id);
        // `friendship-keeps-the-peace`: name the gate that holds back each
        // lapsed friend not renewed this turn. Live King
        // civvis-20261101T081602Z let Mali's friendship lapse at t117 without
        // a line, and Mali declared war at t133; four of the day's King seats
        // lost a lapsed friend to a war with no record of why. Empty `lapsed`
        // off the gene, so nothing is written.
        if self.journal().wants(crate::reasoning::Level::Decision) {
            for &other in lapsed.iter().filter(|other| Some(**other) != partner) {
                if let Some(reason) = self.friendship_block(g, pid, other, keeps_the_peace, &lapsed)
                {
                    let civ = &g.players[other].civ;
                    think!(self.journal(), Diplomacy, Decision,
                        "Not renewing the friendship with {civ}";
                        "friendship-keeps-the-peace: {reason}");
                }
            }
        }
        let Some(partner) = partner else {
            // `score-seat-keeps-friends`: why nobody is asked this turn.
            if keeps_friends && self.journal().wants(crate::reasoning::Level::Detail) {
                let reasons: Vec<String> = g
                    .players
                    .iter()
                    .filter(|other| {
                        other.id != pid
                            && other.alive
                            && !other.is_minor
                            && !other.is_barbarian
                            && g.has_met(pid, other.id)
                    })
                    .filter_map(|other| {
                        self.friendship_block(g, pid, other.id, keeps_the_peace, &lapsed)
                            .map(|reason| format!("{}: {reason}", other.civ))
                    })
                    .collect();
                if !reasons.is_empty() {
                    think!(self.journal(), Diplomacy, Detail,
                        "No friendship to offer";
                        "score-seat-keeps-friends: {}", reasons.join("; "));
                }
            }
            return;
        };
        if g.apply(
            pid,
            &Action::ProposeDeal {
                player: partner,
                give_gold: 0.0,
                request_gold: 0.0,
                open_borders: false,
                friendship: true,
                peace: false,
                alliance: None,
            },
        )
        .is_ok()
            && self.journal().wants(crate::reasoning::Level::Decision)
        {
            let civ = g.players[partner].civ.clone();
            if let Some(power) = self.gated_race_keeps_the_friend(g, pid, partner, &lapsed) {
                self.journal_gated_race_renewal(g, partner, power);
            }
            if keeps_friends {
                let tiles = Self::score_friend_distance(g, pid, partner);
                let power = g.military_power(partner);
                think!(self.journal(), Diplomacy, Decision,
                    "Offering friendship to {civ}";
                    "score-seat-keeps-friends: {}; {tiles} tiles from our nearest city, {power:.0} \
                     power; no rival has declared on us while a declared friendship stood",
                    if lapsed.contains(&partner) { "renewing a lapsed friendship" } else { "the nearest major the host lets us ask" });
            } else if lapsed.contains(&partner) {
                think!(self.journal(), Diplomacy, Decision,
                    "Renewing the friendship with {civ}";
                    "friendship-keeps-the-peace: it lapsed within {FRIENDSHIP_RENEW_WINDOW} turns; \
                     a friend may not declare war on us");
            } else {
                think!(self.journal(), Diplomacy, Decision,
                    "Offering friendship to {civ}";
                    "the strongest neighbour we are at peace with; while a friendship lasts neither of us may declare war");
            }
        }
    }
}

#[cfg(test)]
mod tests;
