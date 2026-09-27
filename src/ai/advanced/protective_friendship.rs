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
//! The live Civilization VI seat does not carry this yet: its order
//! translation (`civvis_orders`) sends a peace deal to the host and skips
//! every other deal, friendship included.

use super::*;

/// Every third turn, staggered by seat, so a seat asks a partner who refused
/// again before long without re-proposing every turn.
const FRIENDSHIP_CADENCE: u32 = 3;

impl AdvancedAi {
    /// Offer a friendship-only deal to the strongest met major at peace with
    /// us that is not already a friend, is not denounced either way, and has
    /// no deal pending with us. Exact no-op with the gene off.
    pub(super) fn propose_protective_friendship(&self, g: &mut Game, pid: usize) {
        if !self.befriend_the_strongest
            || g.turn % FRIENDSHIP_CADENCE != pid as u32 % FRIENDSHIP_CADENCE
        {
            return;
        }
        let denounced = |g: &Game, by: usize, of: usize| {
            g.players[by]
                .denounced_until
                .get(&of)
                .is_some_and(|until| *until > g.turn)
        };
        let partner = g
            .players
            .iter()
            .filter(|other| {
                other.id != pid
                    && other.alive
                    && !other.is_minor
                    && !other.is_barbarian
                    && g.has_met(pid, other.id)
                    && !g.same_team(pid, other.id)
                    && !g.is_at_war(pid, other.id)
                    && !g.are_friends(pid, other.id)
                    && !denounced(g, pid, other.id)
                    && !denounced(g, other.id, pid)
                    && !g.pending_deals.iter().any(|deal| {
                        (deal.from == pid && deal.to == other.id)
                            || (deal.from == other.id && deal.to == pid)
                    })
            })
            .max_by(|a, b| {
                g.military_power(a.id)
                    .total_cmp(&g.military_power(b.id))
                    .then(b.id.cmp(&a.id))
            })
            .map(|other| other.id);
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
            think!(self.journal(), Diplomacy, Decision,
                "Offering friendship to {civ}";
                "the strongest neighbour we are at peace with; while a friendship lasts neither of us may declare war");
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
}
