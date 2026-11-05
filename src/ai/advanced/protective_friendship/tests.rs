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
/// written. A host that still reads DECLARED_FRIEND is named as standing.
#[test]
fn a_lapsed_friend_held_back_is_journalled_with_its_gate() {
    use crate::ai::advanced::VictoryTarget;
    use crate::ai::Ai;
    use crate::reasoning::Journal;
    let run = |gene: bool, state: &str| {
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
            .insert(0, state.to_string());
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
    let (renewed, held) = run(true, "NEUTRAL");
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
    let (renewed, held) = run(false, "NEUTRAL");
    assert!(!renewed);
    assert!(held.is_empty(), "off, nothing is written: {held:?}");
    // The turn before the host ends the friendship it still reads
    // DECLARED_FRIEND: named as standing, not as cold.
    let (renewed, held) = run(true, "DECLARED_FRIEND");
    assert!(!renewed);
    assert_eq!(held.len(), 1, "{held:?}");
    assert!(
        held[0].contains("still counts the friendship as standing"),
        "{}",
        held[0]
    );
}

/// `friendship-refuses-the-racer`: a racer's offer of friendship is
/// refused with the gene and priced as before without it. Native King
/// Domination seat 30700000 accepted France's offer at t128 and t149
/// while France's culture read 45% and 64% and lost to it at t161.
#[test]
fn a_racer_s_offer_of_friendship_is_refused() {
    use crate::ai::advanced::{GrandStrategy, StrategicPlan};
    use crate::game::{DiplomaticDeal, ObservedPublicEmpireStats};
    use std::sync::Arc;
    let mut game = three_majors();
    // Seat 1 one visitor short of the largest other staycation.
    let stats = Arc::make_mut(&mut game.observed_public_empire_stats);
    for (pid, foreign) in [(0, 0), (1, 19), (2, 0)] {
        stats.insert(
            pid,
            ObservedPublicEmpireStats {
                domestic_tourists: Some(20),
                foreign_tourists: Some(foreign),
                ..Default::default()
            },
        );
    }
    let offer = |from: usize| DiplomaticDeal {
        id: 1,
        from,
        to: 0,
        give_gold: 0.0,
        request_gold: 0.0,
        open_borders: false,
        friendship: true,
        peace: false,
        alliance: None,
        defensive_pact: false,
        joint_war_target: None,
        promise: None,
        demand: false,
        expires: game.turn + 1,
    };
    let plan = StrategicPlan {
        strategy: GrandStrategy::Expansion,
        target_player: None,
        target_city: None,
        threatened_city: None,
        desired_cities: 3,
        assessed_turn: game.turn,
        rush: false,
    };
    let mut on = AdvancedAi::new();
    on.enable_friendship_refuses_the_racer();
    let off = AdvancedAi::new();
    assert!(on.friend_is_racing(&game, 1), "fixture: seat 1 is racing");
    assert_eq!(
        on.incoming_deal_value(&game, 0, &offer(1), &plan),
        -1_000.0,
        "on: the racer's offer is refused"
    );
    assert!(
        off.incoming_deal_value(&game, 0, &offer(1), &plan) > 0.0,
        "off: priced as before"
    );
    assert!(
        on.incoming_deal_value(&game, 0, &offer(2), &plan) > 0.0,
        "on: a rival that is not racing is priced as before"
    );
}

#[test]
fn friendship_refuses_the_racer_is_opt_in() {
    opt_in_off_in_both_controllers("friendship-refuses-the-racer", |ai| {
        ai.friendship_refuses_the_racer
    });
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
