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
    fn friend_is_racing(&self, g: &Game, rival: usize) -> bool {
        self.rival_pressure(g, rival).1 >= FRIENDSHIP_RACE_CEILING
            || self.urgent_victory_threat(g, rival)
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
        if !rival
            .observed_diplomatic_state
            .get(&pid)
            .is_none_or(|state| state == "FRIENDLY")
        {
            return Some("their attitude toward us is colder than FRIENDLY, and the host befriends only from FRIENDLY");
        }
        // `friendship-ends-at-the-race`: a rival racing to win is left to the
        // counter once the friendship lapses.
        if self.friendship_ends_at_the_race && self.friend_is_racing(g, other) {
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
        if self.friendship_spares_the_converter
            && self.friendship_would_shield_the_converter(g, pid, other)
        {
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
        let lapsed = if keeps_the_peace && self.befriend_the_strongest {
            self.friendship_lapses(g, pid)
        } else {
            BTreeSet::new()
        };
        if !self.befriend_the_strongest
            || (g.turn % FRIENDSHIP_CADENCE != pid as u32 % FRIENDSHIP_CADENCE && lapsed.is_empty())
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
            if lapsed.contains(&partner) {
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
mod tests {
    use super::super::test_support::opt_in_off_in_both_controllers;
    use super::super::AdvancedAi;
    use crate::game::{Action, Game};

    #[test]
    fn befriend_the_strongest_is_a_native_opt_in_off_in_both_controllers() {
        opt_in_off_in_both_controllers("befriend-the-strongest", |ai| ai.befriend_the_strongest);
    }

    /// Three majors in contact, each with a capital; major 2 fields an army
    /// so it is the strongest. The turn is one on which seat 0 asks.
    fn three_majors() -> Game {
        let mut game = Game::new_full(3, 24, 16, 7_923, 300, 0, false);
        for pid in 0..3 {
            let settler = game
                .player_unit_ids(pid)
                .into_iter()
                .find(|unit| game.units[unit].kind == "settler")
                .expect("every major starts with a settler");
            game.found_city_for(pid, game.units[&settler].pos, None);
            game.remove_unit(settler);
        }
        for pid in 0..3 {
            for other in pid + 1..3 {
                game.record_contact(pid, other);
            }
        }
        let capital = game.cities[&game.player_city_ids(2)[0]].pos;
        for _ in 0..4 {
            game.spawn_test_unit("warrior", 2, capital);
        }
        game.turn = 30;
        game
    }

    #[test]
    fn the_strongest_neighbour_is_offered_a_friendship_and_nothing_else() {
        let mut game = three_majors();
        assert!(game.military_power(2) > game.military_power(1));
        let mut ai = AdvancedAi::new();
        ai.propose_protective_friendship(&mut game, 0);
        assert!(game.pending_deals.is_empty(), "off, the seat asks nobody");

        ai.enable_befriend_the_strongest();
        ai.propose_protective_friendship(&mut game, 0);
        let deal = game
            .pending_deals
            .iter()
            .find(|deal| deal.from == 0)
            .expect("the seat offers a friendship");
        assert_eq!(deal.to, 2, "the strongest neighbour is asked first");
        assert!(deal.friendship);
        assert!(!deal.open_borders && !deal.peace && deal.alliance.is_none());
        assert_eq!((deal.give_gold, deal.request_gold), (0.0, 0.0));

        // A deal is pending with major 2, so the next ask goes to major 1.
        game.turn += 3;
        ai.propose_protective_friendship(&mut game, 0);
        assert!(game
            .pending_deals
            .iter()
            .any(|deal| deal.from == 0 && deal.to == 1 && deal.friendship));
    }

    /// On a live board the host's reading of each leader decides who is
    /// asked: the strongest rival, Neutral toward us, is passed over for a
    /// Friendly one, and nobody is asked while nobody is Friendly.
    #[test]
    fn a_live_seat_asks_only_a_rival_the_host_reads_as_friendly() {
        let mut game = three_majors();
        let mut ai = AdvancedAi::new();
        ai.enable_befriend_the_strongest();
        let attitude = |game: &mut Game, pid: usize, state: &str| {
            game.players[pid]
                .observed_diplomatic_state
                .insert(0, state.to_string());
        };
        attitude(&mut game, 2, "NEUTRAL");
        attitude(&mut game, 1, "UNFRIENDLY");
        ai.propose_protective_friendship(&mut game, 0);
        assert!(game.pending_deals.is_empty(), "no rival is Friendly");

        attitude(&mut game, 1, "FRIENDLY");
        ai.propose_protective_friendship(&mut game, 0);
        let deal = game
            .pending_deals
            .iter()
            .find(|deal| deal.from == 0)
            .expect("the Friendly rival is asked");
        assert_eq!(deal.to, 1, "the Neutral strongest rival is passed over");
    }

    /// `friendship-ends-at-the-race`: the strongest rival, racing to a
    /// Diplomatic Victory at half the points, is passed over for the next.
    #[test]
    fn a_rival_racing_to_win_is_not_befriended() {
        let mut game = three_majors();
        game.players[2].dvp = 10;
        let mut v1 = game.clone();
        let mut ai = AdvancedAi::new();
        ai.enable_befriend_the_strongest();
        ai.propose_protective_friendship(&mut v1, 0);
        assert!(
            v1.pending_deals
                .iter()
                .any(|deal| deal.from == 0 && deal.to == 2),
            "without the gene the racer is asked"
        );

        ai.enable_friendship_ends_at_the_race();
        ai.propose_protective_friendship(&mut game, 0);
        let deal = game
            .pending_deals
            .iter()
            .find(|deal| deal.from == 0)
            .expect("another rival is asked");
        assert_eq!(deal.to, 1, "the racer is passed over");
    }

    /// `friendship-only-with-the-stronger`: a rival weaker than us is not
    /// asked, so once the stronger one has an offer pending nobody else is.
    #[test]
    fn only_a_rival_at_least_as_strong_as_us_is_befriended() {
        let mut game = three_majors();
        let capital = game.cities[&game.player_city_ids(0)[0]].pos;
        for _ in 0..2 {
            game.spawn_test_unit("warrior", 0, capital);
        }
        assert!(game.military_power(0) > game.military_power(1));
        assert!(game.military_power(0) < game.military_power(2));
        let mut ai = AdvancedAi::new();
        ai.enable_befriend_the_strongest();
        ai.enable_friendship_only_with_the_stronger();
        ai.propose_protective_friendship(&mut game, 0);
        assert!(game
            .pending_deals
            .iter()
            .any(|deal| deal.from == 0 && deal.to == 2 && deal.friendship));

        game.turn += 3;
        ai.propose_protective_friendship(&mut game, 0);
        assert!(
            !game.pending_deals.iter().any(|deal| deal.to == 1),
            "the weaker rival is left to the conquest"
        );
    }

    /// `friendship-spares-the-target`: the strongest rival, named by the
    /// plan as the campaign's target, is passed over for the next.
    #[test]
    fn the_campaign_target_is_not_befriended() {
        let mut game = three_majors();
        let mut ai = AdvancedAi::new();
        ai.enable_befriend_the_strongest();
        ai.plan = Some(super::super::StrategicPlan {
            strategy: super::super::GrandStrategy::Conquest,
            target_player: Some(2),
            target_city: None,
            threatened_city: None,
            desired_cities: 3,
            assessed_turn: game.turn,
            rush: false,
        });
        let mut v1 = game.clone();
        ai.propose_protective_friendship(&mut v1, 0);
        assert!(
            v1.pending_deals
                .iter()
                .any(|deal| deal.from == 0 && deal.to == 2),
            "without the gene the target is asked"
        );

        ai.enable_friendship_spares_the_target();
        ai.propose_protective_friendship(&mut game, 0);
        let deal = game
            .pending_deals
            .iter()
            .find(|deal| deal.from == 0)
            .expect("another rival is asked");
        assert_eq!(deal.to, 1, "the campaign target is passed over");
    }

    /// `friendship-spares-the-prey`: with the strongest rival befriended,
    /// a rival under half our power is not asked next.
    #[test]
    fn a_rival_weak_enough_to_be_prey_is_not_befriended() {
        let mut game = three_majors();
        let capital = game.cities[&game.player_city_ids(0)[0]].pos;
        for _ in 0..3 {
            game.spawn_test_unit("warrior", 0, capital);
        }
        assert!(
            game.military_power(1) < super::FRIENDSHIP_PREY_RATIO * game.military_power(0),
            "fixture: major 1 is prey"
        );
        let mut ai = AdvancedAi::new();
        ai.enable_befriend_the_strongest();
        let mut off = game.clone();
        ai.propose_protective_friendship(&mut off, 0);
        off.turn += 3;
        ai.propose_protective_friendship(&mut off, 0);
        assert!(
            off.pending_deals
                .iter()
                .any(|deal| deal.from == 0 && deal.to == 1),
            "without the gene the prey is asked next"
        );

        ai.enable_friendship_spares_the_prey();
        ai.propose_protective_friendship(&mut game, 0);
        game.turn += 3;
        ai.propose_protective_friendship(&mut game, 0);
        assert!(
            !game.pending_deals.iter().any(|deal| deal.to == 1),
            "the prey is left to the conquest"
        );
    }

    /// Found `faith` for `founder` and have the capitals of `followers`
    /// follow it.
    fn found_faith(game: &mut Game, founder: usize, faith: &str, followers: &[usize]) {
        game.players[founder].religion = Some(faith.to_string());
        for &pid in followers {
            let capital = game.player_city_ids(pid)[0];
            game.cities
                .get_mut(&capital)
                .expect("the capital stands")
                .pressure
                .insert(faith.to_string(), 100.0);
        }
    }

    /// The first rival seat 0 offers a friendship to this turn, if any.
    fn first_offer(ai: &AdvancedAi, game: &Game) -> Option<usize> {
        let mut game = game.clone();
        ai.propose_protective_friendship(&mut game, 0);
        game.pending_deals
            .iter()
            .find(|deal| deal.from == 0 && deal.friendship)
            .map(|deal| deal.to)
    }

    /// `friendship-spares-the-converter`: the strongest rival founded the
    /// faith most cities follow, so it is passed over for the next, whose
    /// faith nobody follows yet.
    #[test]
    fn the_founder_whose_faith_leads_the_race_is_not_befriended() {
        let mut game = three_majors();
        found_faith(&mut game, 2, "Christianity", &[2]);
        found_faith(&mut game, 1, "Islam", &[]);
        let mut ai = AdvancedAi::new();
        ai.enable_befriend_the_strongest();
        assert_eq!(
            first_offer(&ai, &game),
            Some(2),
            "off, the strongest is asked"
        );

        ai.enable_friendship_spares_the_converter();
        assert_eq!(
            first_offer(&ai, &game),
            Some(1),
            "the race leader is passed over for a founder nobody follows"
        );

        // Without a Religious Victory there is no faith to fight.
        game.victory_conditions.religious = false;
        assert_eq!(first_offer(&ai, &game), Some(2));
    }

    /// `friendship-spares-the-converter`: a trailing faith that already holds
    /// a city of ours is passed over too, so with the leader also passed over
    /// nobody is asked.
    #[test]
    fn a_founder_whose_faith_holds_our_city_is_not_befriended() {
        let mut game = three_majors();
        found_faith(&mut game, 2, "Christianity", &[1, 2]);
        found_faith(&mut game, 1, "Islam", &[0]);
        assert!(
            game.cities_following_religion(1) < game.cities_following_religion(2),
            "fixture: Islam trails"
        );
        let mut ai = AdvancedAi::new();
        ai.enable_befriend_the_strongest();
        ai.enable_friendship_spares_the_converter();
        assert_eq!(first_offer(&ai, &game), None);

        // Our own founded faith is no faith to fight.
        game.players[0].religion = Some("Islam".to_string());
        game.players[1].religion = None;
        assert_eq!(first_offer(&ai, &game), Some(1));
    }

    #[test]
    fn an_accepted_friendship_forbids_the_war_until_it_lapses() {
        let mut game = three_majors();
        let mut ai = AdvancedAi::new();
        ai.enable_befriend_the_strongest();
        ai.propose_protective_friendship(&mut game, 0);
        let deal = game.pending_deals[0].id;
        game.current = 2;
        game.apply(2, &Action::AcceptDeal { deal })
            .expect("the partner accepts");
        assert!(game.are_friends(0, 2));
        assert!(game.apply(2, &Action::DeclareWar { player: 0 }).is_err());

        // Nobody else to ask while the only other major is also a friend.
        game.players[0].friends_until.insert(1, game.turn + 30);
        game.players[1].friends_until.insert(0, game.turn + 30);
        game.pending_deals.clear();
        game.current = 0;
        game.turn += 3;
        ai.propose_protective_friendship(&mut game, 0);
        assert!(game.pending_deals.is_empty());
    }

    /// `friendship-keeps-the-peace`: a weaker rival's friendship stands at
    /// turn 30 and lapses at 31, an off-cadence turn; a Science seat renews
    /// it at once despite `friendship-only-with-the-stronger`, and neither a
    /// seat without the gene nor a Domination seat does.
    #[test]
    fn a_lapsed_friendship_is_renewed_at_once_off_domination() {
        use crate::ai::advanced::VictoryTarget;
        let run = |target: VictoryTarget, gene: bool| {
            let mut game = three_majors();
            assert!(game.military_power(1) < game.military_power(2));
            let mut ai = AdvancedAi::targeting(target);
            ai.enable_befriend_the_strongest();
            ai.enable_friendship_only_with_the_stronger();
            if gene {
                ai.enable_friendship_keeps_the_peace();
            }
            // Turn 30: the friendship with seat 1 stands until 31.
            game.players[0].friends_until.insert(1, 31);
            game.players[1].friends_until.insert(0, 31);
            ai.propose_protective_friendship(&mut game, 0);
            game.pending_deals.clear();
            // Turn 31: it lapsed, and the live mirror drops it.
            game.turn = 31;
            game.players[0].friends_until.remove(&1);
            game.players[1].friends_until.remove(&0);
            ai.propose_protective_friendship(&mut game, 0);
            game.pending_deals
                .iter()
                .find(|deal| deal.from == 0 && deal.friendship)
                .map(|deal| deal.to)
        };
        assert_eq!(
            run(VictoryTarget::Science, true),
            Some(1),
            "renewed at once"
        );
        assert_eq!(
            run(VictoryTarget::Science, false),
            None,
            "off: the cadence waits"
        );
        assert_eq!(
            run(VictoryTarget::Domination, true),
            None,
            "a Domination seat keeps the Domination filters and the cadence"
        );
    }

    /// `friendship-keeps-the-peace`: a lapsed friend that is held back is
    /// journalled with the gate that holds it. Seat 1's friendship lapses at
    /// 31 while the host reads its attitude toward us as NEUTRAL: it is not
    /// renewed, and the line names the attitude; off the gene, nothing is
    /// written.
    #[test]
    fn a_lapsed_friend_held_back_is_journalled_with_its_gate() {
        use crate::ai::advanced::VictoryTarget;
        use crate::ai::Ai;
        use crate::reasoning::Journal;
        let run = |gene: bool| {
            let mut game = three_majors();
            let mut ai = AdvancedAi::targeting(VictoryTarget::Science);
            let journal = Journal::recording();
            ai.attach_journal(journal.handle());
            ai.enable_befriend_the_strongest();
            if gene {
                ai.enable_friendship_keeps_the_peace();
            }
            game.players[0].friends_until.insert(1, 31);
            game.players[1].friends_until.insert(0, 31);
            ai.propose_protective_friendship(&mut game, 0);
            game.pending_deals.clear();
            game.turn = 31;
            game.players[0].friends_until.remove(&1);
            game.players[1].friends_until.remove(&0);
            game.players[1]
                .observed_diplomatic_state
                .insert(0, "NEUTRAL".to_string());
            ai.propose_protective_friendship(&mut game, 0);
            let renewed = game
                .pending_deals
                .iter()
                .any(|deal| deal.from == 0 && deal.to == 1 && deal.friendship);
            let held: Vec<String> = journal
                .since(0)
                .thoughts
                .iter()
                .filter(|thought| thought.headline.starts_with("Not renewing the friendship"))
                .map(|thought| thought.detail.clone())
                .collect();
            (renewed, held)
        };
        let (renewed, held) = run(true);
        assert!(!renewed, "a NEUTRAL leader is not asked");
        assert_eq!(
            held.len(),
            1,
            "one line for the one lapsed friend: {held:?}"
        );
        assert!(
            held[0].contains("colder than FRIENDLY"),
            "the line names the gate: {}",
            held[0]
        );
        let (renewed, held) = run(false);
        assert!(!renewed);
        assert!(held.is_empty(), "off, nothing is written: {held:?}");
    }

    #[test]
    fn friendship_keeps_the_peace_is_opt_in() {
        opt_in_off_in_both_controllers("friendship-keeps-the-peace", |ai| {
            ai.friendship_keeps_the_peace
        });
    }

    /// `friendship-keeps-the-peace`: our own pending alliance proposal to the
    /// lapsed friend does not hold back its renewal; a pending deal the friend
    /// proposed, or one without an alliance, still does.
    #[test]
    fn a_pending_alliance_proposal_does_not_block_the_renewal() {
        use crate::ai::advanced::VictoryTarget;
        use crate::game::DiplomaticDeal;
        let run = |pending: Option<(usize, usize, bool)>| {
            let mut game = three_majors();
            let mut ai = AdvancedAi::targeting(VictoryTarget::Religion);
            ai.enable_befriend_the_strongest();
            ai.enable_friendship_keeps_the_peace();
            game.players[0].friends_until.insert(1, 31);
            game.players[1].friends_until.insert(0, 31);
            ai.propose_protective_friendship(&mut game, 0);
            game.pending_deals.clear();
            game.turn = 31;
            game.players[0].friends_until.remove(&1);
            game.players[1].friends_until.remove(&0);
            if let Some((from, to, alliance)) = pending {
                let id = game.next_id;
                game.next_id += 1;
                game.pending_deals.push(DiplomaticDeal {
                    id,
                    from,
                    to,
                    give_gold: 0.0,
                    request_gold: 0.0,
                    // An alliance proposal bundles the friendship, as the
                    // alliance desk's does; the other pending deal is a
                    // passage sale, which carries neither.
                    open_borders: !alliance,
                    friendship: alliance,
                    peace: false,
                    alliance: alliance.then(|| "research".to_string()),
                    defensive_pact: false,
                    joint_war_target: None,
                    promise: None,
                    demand: false,
                    expires: game.turn + 10,
                });
            }
            ai.propose_protective_friendship(&mut game, 0);
            game.pending_deals.iter().any(|deal| {
                deal.from == 0 && deal.to == 1 && deal.friendship && deal.alliance.is_none()
            })
        };
        assert!(run(None), "the renewal, as before");
        assert!(
            run(Some((0, 1, true))),
            "our own pending alliance proposal does not hold it back"
        );
        assert!(
            !run(Some((1, 0, true))),
            "a deal the friend proposed still does"
        );
        assert!(
            !run(Some((0, 1, false))),
            "nor does a pending deal without an alliance (a passage sale)"
        );
    }
}
