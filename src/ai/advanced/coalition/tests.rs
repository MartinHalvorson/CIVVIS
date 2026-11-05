use super::super::test_support::opt_in_off_in_both_controllers;
use super::super::{AdvancedAi, GrandStrategy, StrategicPlan};
use super::*;
use crate::game::{Action, Game};

#[test]
fn coalition_before_war_is_a_native_opt_in_off_in_both_controllers() {
    opt_in_off_in_both_controllers("coalition-before-war", |ai| ai.coalition_before_war);
    opt_in_off_in_both_controllers("coalition-before-war-2", |ai| ai.coalition_before_war_2);
    opt_in_off_in_both_controllers("coalition-before-war-3", |ai| ai.coalition_before_war_3);

    let mut ai = AdvancedAi::new();
    ai.enable_coalition_before_war();
    assert!(ai.coalition_before_war);
    assert!(!ai.coalition_before_war_2);
    assert!(!ai.coalition_before_war_3);
    ai.enable_coalition_before_war_2();
    assert!(!ai.coalition_before_war);
    assert!(ai.coalition_before_war_2);
    assert!(!ai.coalition_before_war_3);
    ai.enable_coalition_before_war_3();
    assert!(!ai.coalition_before_war);
    assert!(!ai.coalition_before_war_2);
    assert!(ai.coalition_before_war_3);
}

/// Open land at exactly `distance` from `anchor`, for seating a city.
fn open_ground_at(game: &Game, anchor: crate::Pos, distance: i32) -> crate::Pos {
    game.map
        .tiles
        .keys()
        .copied()
        .filter(|position| {
            game.wdist(*position, anchor) == distance
                && game.rules.is_passable(&game.map.tiles[position])
                && !game.rules.is_water(&game.map.tiles[position])
                && game.map.tiles[position].owner_city.is_none()
                && game.city_at(*position).is_none()
        })
        .min()
        .expect("open ground at the distance")
}

/// Three majors with a capital each and two city-states. Major 1 is the
/// target; major 2 is seated as its neighbour (a second city six tiles
/// from the target's capital); the first city-state four tiles from the
/// target's capital, the second as far from it as the map allows.
fn coalition_board() -> (Game, StrategicPlan, usize, usize) {
    let mut game = Game::new_full(3, 24, 16, 7_923, 300, 2, false);
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
    for player in game.players.iter_mut().filter(|p| !p.is_minor) {
        player.civics.insert(crate::name!("civil_service"));
        player.civics.insert(crate::name!("foreign_trade"));
    }
    let target_capital = game.cities[&game.player_city_ids(1)[0]].pos;
    let border = open_ground_at(&game, target_capital, 6);
    game.found_city_for(2, border, None);
    let minors: Vec<usize> = game
        .players
        .iter()
        .filter(|p| p.is_minor && !p.is_barbarian)
        .map(|p| p.id)
        .collect();
    assert_eq!(minors.len(), 2);
    for minor in &minors {
        for cid in game.player_city_ids(*minor) {
            game.cities.remove(&cid);
        }
        for unit in game.player_unit_ids(*minor) {
            game.remove_unit(unit);
        }
    }
    let near_seat = open_ground_at(&game, target_capital, 4);
    game.found_city_for(minors[0], near_seat, None);
    let farthest = game
        .map
        .tiles
        .keys()
        .copied()
        .filter(|position| {
            game.rules.is_passable(&game.map.tiles[position])
                && !game.rules.is_water(&game.map.tiles[position])
                && game.map.tiles[position].owner_city.is_none()
                && game.city_at(*position).is_none()
        })
        .max_by_key(|position| (game.wdist(*position, target_capital), *position))
        .expect("land somewhere far");
    assert!(game.wdist(farthest, target_capital) > COALITION_CITY_STATE_RADIUS);
    game.found_city_for(minors[1], farthest, None);
    for pid in 0..3 {
        game.players[pid].met.insert(minors[0]);
        game.players[pid].met.insert(minors[1]);
        game.players[minors[0]].met.insert(pid);
        game.players[minors[1]].met.insert(pid);
    }
    game.turn = 60;
    game.current = 0;
    let plan = StrategicPlan {
        strategy: GrandStrategy::Conquest,
        target_player: Some(1),
        target_city: Some(game.player_city_ids(1)[0]),
        threatened_city: None,
        desired_cities: 3,
        assessed_turn: game.turn,
        rush: false,
    };
    (game, plan, minors[0], minors[1])
}

fn coalition_ai() -> AdvancedAi {
    let mut ai = AdvancedAi::new();
    ai.enable_coalition_before_war();
    ai
}

fn coalition_v2_ai() -> AdvancedAi {
    let mut ai = AdvancedAi::new();
    ai.enable_coalition_before_war_2();
    ai
}

fn coalition_v3_ai() -> AdvancedAi {
    let mut ai = AdvancedAi::new();
    ai.enable_coalition_before_war_3();
    ai
}

/// The window opens on a peaceful Conquest target, stays shut with the
/// gene off, and closes the turn the war starts.
#[test]
fn the_window_opens_on_a_peaceful_target_and_closes_at_war() {
    let (mut game, plan, _, _) = coalition_board();
    let mut stock = AdvancedAi::new();
    assert_eq!(stock.coalition_target(&game, 0, &plan), None);
    stock.coalition_observe(&mut game, 0, &plan);
    assert!(stock.coalition.is_none());

    let mut ai = coalition_ai();
    assert_eq!(ai.coalition_target(&game, 0, &plan), Some(1));
    ai.coalition_observe(&mut game, 0, &plan);
    let opened = ai.coalition.clone().expect("the window is open");
    assert_eq!((opened.target, opened.opened), (1, 60));
    assert_eq!(game.players[0].counters.get("coalition:opened"), Some(&1));
    assert_eq!(ai.coalition_neighbours(&game, 0, 1), vec![2]);

    // A second observation of the same target keeps the window.
    game.turn = 61;
    ai.coalition_observe(&mut game, 0, &plan);
    assert_eq!(ai.coalition.as_ref().map(|c| c.opened), Some(60));

    // A plan that is not a war has no window; the war's start closes it.
    let peaceful = StrategicPlan {
        strategy: GrandStrategy::Science,
        ..plan.clone()
    };
    assert_eq!(ai.coalition_target(&game, 0, &peaceful), None);
    game.at_war.insert((0, 1));
    assert_eq!(ai.coalition_target(&game, 0, &plan), None);
    ai.coalition_observe(&mut game, 0, &plan);
    assert!(ai.coalition.is_none());
}

/// One military alliance is proposed to the target's neighbour, with
/// friendship bundled; never to the target, never twice while the first
/// is pending, and never with the gene off.
#[test]
fn an_alliance_is_proposed_to_the_targets_neighbour_and_never_to_the_target() {
    let (mut game, plan, _, _) = coalition_board();
    let mut stock = AdvancedAi::new();
    stock.coalition_observe(&mut game, 0, &plan);
    stock.coalition_alliance_step(&mut game, 0);
    assert!(game.pending_deals.is_empty());

    let mut ai = coalition_ai();
    ai.coalition_observe(&mut game, 0, &plan);
    ai.coalition_alliance_step(&mut game, 0);
    let proposals: Vec<_> = game
        .pending_deals
        .iter()
        .filter(|deal| deal.from == 0)
        .cloned()
        .collect();
    assert_eq!(proposals.len(), 1);
    let proposal = &proposals[0];
    assert_eq!(proposal.to, 2);
    assert_eq!(proposal.alliance.as_deref(), Some("military"));
    assert!(proposal.friendship);
    assert_eq!(
        game.players[0].counters.get("coalition:alliances_proposed"),
        Some(&1)
    );
    assert_eq!(
        ai.coalition.as_ref().and_then(|c| c.alliance_asked.get(&2)),
        Some(&60)
    );
    // Pending: no second proposal this turn.
    ai.coalition_alliance_step(&mut game, 0);
    assert_eq!(
        game.pending_deals
            .iter()
            .filter(|deal| deal.from == 0)
            .count(),
        1
    );

    // The neighbour accepts: friendship and a military alliance stand,
    // and the desk asks for nothing more of them.
    let id = proposal.id;
    game.current = 2;
    game.apply(2, &Action::AcceptDeal { deal: id })
        .expect("the neighbour accepts");
    game.current = 0;
    assert_eq!(
        game.alliance_with(0, 2)
            .map(|alliance| alliance.kind.as_str()),
        Some("military")
    );
    assert!(game.are_friends(0, 2));
    game.turn = 75;
    ai.coalition_alliance_step(&mut game, 0);
    assert!(game.pending_deals.iter().all(|deal| deal.from != 0));
}

/// A refused neighbour is asked again only after the re-ask interval,
/// and the kind falls to the next free one when `military` is taken.
#[test]
fn a_refusal_waits_for_the_reask_interval_and_a_taken_kind_falls_through() {
    let (mut game, plan, _, _) = coalition_board();
    let mut ai = coalition_ai();
    ai.coalition_observe(&mut game, 0, &plan);
    ai.coalition_alliance_step(&mut game, 0);
    let id = game
        .pending_deals
        .iter()
        .find(|deal| deal.from == 0)
        .unwrap()
        .id;
    game.current = 2;
    game.apply(2, &Action::RejectDeal { deal: id })
        .expect("refused");
    game.current = 0;
    assert!(game.pending_deals.iter().all(|deal| deal.from != 0));
    game.turn = 60 + COALITION_REASK_TURNS - 1;
    ai.coalition_alliance_step(&mut game, 0);
    assert!(game.pending_deals.iter().all(|deal| deal.from != 0));
    // Meanwhile our military alliance went to somebody else: the next
    // free kind is asked for.
    game.players[0].alliances.insert(
        1,
        crate::game::AllianceState {
            kind: "military".to_string(),
            points: 0.0,
            level: 1,
            ends: game.turn + 30,
        },
    );
    game.turn = 60 + COALITION_REASK_TURNS;
    ai.coalition_alliance_step(&mut game, 0);
    let proposal = game
        .pending_deals
        .iter()
        .find(|deal| deal.from == 0)
        .unwrap();
    assert_eq!(proposal.to, 2);
    assert_eq!(proposal.alliance.as_deref(), Some("economic"));
}

/// The envoy term: a city-state beside the target is worth envoys, most
/// when the target holds it, nothing beyond the radius, nothing outside
/// the window, nothing with the gene off.
#[test]
fn a_city_state_beside_the_target_is_worth_envoys_most_when_the_target_holds_it() {
    let (mut game, plan, near, far) = coalition_board();
    let mut stock = AdvancedAi::new();
    stock.coalition_observe(&mut game, 0, &plan);
    assert_eq!(stock.coalition_city_state_bonus(&game, 0, near, 2), 0);

    let mut ai = coalition_ai();
    // No window yet: nothing.
    assert_eq!(ai.coalition_city_state_bonus(&game, 0, near, 2), 0);
    ai.coalition_observe(&mut game, 0, &plan);
    let open = ai.coalition_city_state_bonus(&game, 0, near, 2);
    assert_eq!(
        open,
        (i64::from(COALITION_CITY_STATE_RADIUS + 1 - 4) * COALITION_CITY_STATE_PER_TILE
            + COALITION_OPEN_CITY_STATE)
            / 2
    );
    assert_eq!(ai.coalition_city_state_bonus(&game, 0, far, 2), 0);

    // The target takes the suzerainty: its client, our front.
    game.players[1].envoys_free = 3;
    for _ in 0..3 {
        game.current = 1;
        game.apply(1, &Action::SendEnvoy { player: near })
            .expect("the target places an envoy");
    }
    game.current = 0;
    assert_eq!(game.suzerain_of(near), Some(1));
    let client = ai.coalition_city_state_bonus(&game, 0, near, 2);
    assert!(client > open, "{client} > {open}");
    assert_eq!(
        client,
        (i64::from(COALITION_CITY_STATE_RADIUS + 1 - 4) * COALITION_CITY_STATE_PER_TILE
            + COALITION_TARGET_CLIENT)
            / 2
    );
    // Amortised over the envoys still needed.
    assert_eq!(
        ai.coalition_city_state_bonus(&game, 0, near, 4),
        client * 2 / 4
    );

    // A third party's client is proximity only.
    game.players[2].envoys_free = 4;
    for _ in 0..4 {
        game.current = 2;
        game.apply(2, &Action::SendEnvoy { player: near })
            .expect("the neighbour places an envoy");
    }
    game.current = 0;
    assert_eq!(game.suzerain_of(near), Some(2));
    assert_eq!(
        ai.coalition_city_state_bonus(&game, 0, near, 2),
        i64::from(COALITION_CITY_STATE_RADIUS + 1 - 4) * COALITION_CITY_STATE_PER_TILE / 2
    );
}

/// Version two refuses speculative setup, then asks a credible partner
/// only for the military alliance and values only a target-held client.
/// At the strike it invites once and declares next turn after a refusal.
#[test]
fn v2_limits_setup_and_invites_one_credible_partner_once() {
    let (mut game, plan, near, _) = coalition_board();
    let mut ai = coalition_v2_ai();
    ai.coalition_observe(&mut game, 0, &plan);

    // A merely legal neighbour gets no speculative alliance, and an open
    // city-state gets no proximity-only Envoy premium.
    ai.coalition_alliance_step(&mut game, 0);
    assert!(game.pending_deals.is_empty());
    assert_eq!(ai.coalition_city_state_bonus(&game, 0, near, 1), 0);
    assert!(!ai.coalition_credible_partner(&game, 0, 2, 1));

    // Unseating the target from its own nearby client is direct value.
    game.players[1].envoys_free = 3;
    for _ in 0..3 {
        game.current = 1;
        game.apply(1, &Action::SendEnvoy { player: near })
            .expect("the target places an envoy");
    }
    game.current = 0;
    assert_eq!(game.suzerain_of(near), Some(1));
    assert!(ai.coalition_city_state_bonus(&game, 0, near, 1) > 0);

    // A resentful neighbour with enough combined power is credible. V2
    // asks it only for the military alliance, never a fallback kind.
    game.players[2].grievances.insert(1, 40.0);
    let home = game.cities[&game.player_city_ids(2)[0]].pos;
    for _ in 0..6 {
        game.spawn_test_unit("warrior", 2, home);
    }
    assert!(ai.coalition_credible_partner(&game, 0, 2, 1));
    ai.coalition_alliance_step(&mut game, 0);
    let alliance = game
        .pending_deals
        .iter()
        .find(|deal| deal.from == 0 && deal.to == 2)
        .expect("the credible neighbour gets an alliance proposal");
    assert_eq!(alliance.alliance.as_deref(), Some("military"));
    let alliance_id = alliance.id;
    game.current = 2;
    game.apply(2, &Action::RejectDeal { deal: alliance_id })
        .expect("the alliance can be refused");
    game.current = 0;

    assert!(ai.coalition_invites_before_declaring(&mut game, 0, 1));
    let invitations: Vec<_> = game
        .pending_deals
        .iter()
        .filter(|deal| deal.from == 0 && deal.joint_war_target == Some(1))
        .collect();
    assert_eq!(invitations.len(), 1);
    let id = invitations[0].id;

    game.current = 2;
    game.apply(2, &Action::RejectDeal { deal: id })
        .expect("the invitation can be refused");
    game.current = 0;
    game.turn += 1;
    assert!(!ai.coalition_invites_before_declaring(&mut game, 0, 1));
    assert!(game.pending_deals.iter().all(|deal| deal.from != 0));
}

/// Version three asks only a neighbour that already has the target at
/// war. The resulting level-one military alliance is enough for the
/// engine's immediate common-enemy combat bonus; it neither spends Envoys
/// nor waits at the strike for a Joint War reply.
#[test]
fn v3_allies_with_an_active_front_without_holding_the_declaration() {
    let (mut game, plan, near, _) = coalition_board();
    let mut ai = coalition_v3_ai();
    ai.coalition_observe(&mut game, 0, &plan);

    // A peaceful neighbour is not a v3 candidate, even though it is
    // legally adjacent to the target.
    ai.coalition_alliance_step(&mut game, 0);
    assert!(game.pending_deals.is_empty());

    // Even an active front is held until our treaty with the intended
    // target expires: otherwise an alliance would consume the military
    // slot while the declaration remains illegal.
    game.at_war.insert((1, 2));
    game.peace_treaties.insert((0, 1), game.turn + 1);
    ai.coalition_alliance_step(&mut game, 0);
    assert!(game.pending_deals.is_empty());
    game.peace_treaties.clear();

    // The neighbour is now fighting the target. V3 makes exactly one
    // military proposal, and does not run either of v2's costly hooks.
    ai.coalition_alliance_step(&mut game, 0);
    let proposal = game
        .pending_deals
        .iter()
        .find(|deal| deal.from == 0 && deal.to == 2)
        .expect("the existing target front receives the alliance offer")
        .clone();
    assert_eq!(proposal.alliance.as_deref(), Some("military"));
    assert_eq!(proposal.joint_war_target, None);
    assert_eq!(ai.coalition_city_state_bonus(&game, 0, near, 1), 0);
    assert!(!ai.coalition_invites_before_declaring(&mut game, 0, 1));
    ai.coalition_alliance_step(&mut game, 0);
    assert_eq!(
        game.pending_deals
            .iter()
            .filter(|deal| deal.from == 0 && deal.to == 2)
            .count(),
        1,
        "a v3 window does not re-ask its active-front partner"
    );

    // Once our declaration begins, accepting that already-written deal
    // gives our Warrior the military-alliance +5 against the target.
    // `melee_exchange_strengths` is the engine's combat arithmetic, so
    // this checks the actual bonus rather than duplicating it here.
    game.at_war.insert((0, 1));
    let attacker_pos = game.cities[&game.player_city_ids(0)[0]].pos;
    let defender_pos = game.cities[&game.player_city_ids(1)[0]].pos;
    let attacker = game.spawn_test_unit("warrior", 0, attacker_pos);
    let defender = game.spawn_test_unit("warrior", 1, defender_pos);
    let before = game
        .melee_exchange_strengths(attacker, defender)
        .expect("two live Warriors")
        .0;
    game.current = 2;
    game.apply(2, &Action::AcceptDeal { deal: proposal.id })
        .expect("the active-front neighbour accepts the military alliance");
    game.current = 0;
    let after = game
        .melee_exchange_strengths(attacker, defender)
        .expect("the Warriors remain live")
        .0;
    assert_eq!(after, before + 5.0);
}

/// The strike: every eligible neighbour is invited to a joint war and the
/// declaration held while the answer is due; refused, the desk declares
/// alone; nothing with the gene off.
#[test]
fn the_strike_invites_the_neighbours_and_holds_for_the_answer() {
    let (mut game, plan, _, _) = coalition_board();
    let mut stock = AdvancedAi::new();
    stock.coalition_observe(&mut game, 0, &plan);
    assert!(!stock.coalition_invites_before_declaring(&mut game, 0, 1));
    assert!(game.pending_deals.is_empty());

    let mut ai = coalition_ai();
    ai.coalition_observe(&mut game, 0, &plan);
    assert!(ai.coalition_invites_before_declaring(&mut game, 0, 1));
    let invitations: Vec<_> = game
        .pending_deals
        .iter()
        .filter(|deal| deal.from == 0 && deal.joint_war_target == Some(1))
        .collect();
    assert_eq!(invitations.len(), 1);
    assert_eq!(invitations[0].to, 2);
    let id = invitations[0].id;
    assert_eq!(
        game.players[0]
            .counters
            .get("coalition:joint_wars_proposed"),
        Some(&1)
    );
    assert_eq!(
        game.players[0].counters.get("coalition:held_declarations"),
        Some(&1)
    );
    // Still unanswered next turn: held once more, no second invitation.
    game.turn = 61;
    assert!(ai.coalition_invites_before_declaring(&mut game, 0, 1));
    assert_eq!(
        game.pending_deals
            .iter()
            .filter(|deal| deal.from == 0)
            .count(),
        1
    );
    assert_eq!(
        game.players[0].counters.get("coalition:held_declarations"),
        Some(&2)
    );
    // Refused: the desk declares alone, and does not ask again yet.
    game.current = 2;
    game.apply(2, &Action::RejectDeal { deal: id })
        .expect("refused");
    game.current = 0;
    assert!(!ai.coalition_invites_before_declaring(&mut game, 0, 1));
    assert!(game.pending_deals.iter().all(|deal| deal.from != 0));
    // Patience runs out on an unanswered invitation too.
    game.turn = 60 + COALITION_REASK_TURNS;
    assert!(ai.coalition_invites_before_declaring(&mut game, 0, 1));
    let again = game
        .pending_deals
        .iter()
        .find(|deal| deal.from == 0)
        .unwrap();
    assert_eq!(again.joint_war_target, Some(1));
    game.turn += COALITION_JOINT_WAR_PATIENCE;
    assert!(!ai.coalition_invites_before_declaring(&mut game, 0, 1));
}

/// An accepted invitation starts the war for both of us at once.
#[test]
fn an_accepted_invitation_opens_the_war_on_two_fronts() {
    let (mut game, plan, _, _) = coalition_board();
    // A neighbour who resents the target and, with us, outweighs it.
    game.players[2].grievances.insert(1, 40.0);
    let home = game.cities[&game.player_city_ids(2)[0]].pos;
    for _ in 0..3 {
        game.spawn_test_unit("warrior", 2, home);
    }
    let mut ai = coalition_ai();
    ai.coalition_observe(&mut game, 0, &plan);
    assert!(ai.coalition_invites_before_declaring(&mut game, 0, 1));
    let id = game
        .pending_deals
        .iter()
        .find(|deal| deal.from == 0)
        .unwrap()
        .id;
    game.current = 2;
    game.apply(2, &Action::AcceptDeal { deal: id })
        .expect("the neighbour accepts the joint war");
    game.current = 0;
    assert!(game.is_at_war(0, 1));
    assert!(game.is_at_war(2, 1));
    assert!(!game.is_at_war(0, 2));
    // The window closes with the war on.
    ai.coalition_observe(&mut game, 0, &plan);
    assert!(ai.coalition.is_none());
    assert!(!ai.coalition_invites_before_declaring(&mut game, 0, 1));
}
