use super::*;
use crate::ai::advanced::{genes, VictoryTarget};
use crate::game::{Action, AllianceState};
use std::sync::Arc;

/// Four majors, one city each, every pair met and at peace, Civil Service and
/// Scientific Theory on every tree so a Research Alliance is legal between
/// any two of them. Turn 60 is inside the opening band by city count, which
/// the route tests move out of deliberately.
fn board() -> Game {
    let mut g = Game::new_full(4, 34, 24, 79_208, 250, 0, false);
    for pid in 0..4 {
        g.current = pid;
        let unit = g
            .player_unit_ids(pid)
            .into_iter()
            .find(|uid| g.units[uid].kind == "settler")
            .expect("every major starts with a settler");
        g.apply(pid, &Action::FoundCity { unit }).unwrap();
        g.players[pid].gold = 1_000.0;
        g.players[pid].civics.insert(crate::name!("early_empire"));
        g.players[pid].civics.insert(crate::name!("civil_service"));
        g.players[pid]
            .techs
            .insert(crate::name!("scientific_theory"));
    }
    g.current = 0;
    g.turn = 60;
    g.at_war.clear();
    for other in 1..4 {
        g.record_contact(0, other);
    }
    g.record_contact(1, 2);
    g.record_contact(1, 3);
    g.record_contact(2, 3);
    g
}

fn ally_ai() -> AdvancedAi {
    let mut ai = AdvancedAi::targeting(VictoryTarget::Science);
    ai.enable_research_alliance_first();
    ai
}

fn stock_ai() -> AdvancedAi {
    AdvancedAi::targeting(VictoryTarget::Science)
}

/// Strip an empire down to no cities, so its science is zero and it falls
/// under [`ALLY_MIN_SCIENCE_SHARE`] without touching anybody else's board.
fn silence_science(g: &mut Game, pid: usize) {
    for cid in g.player_city_ids(pid) {
        g.cities.remove(&cid);
    }
}

/// Make `rival` the culture threat `culture_trade_threats` reads.
fn make_culture_threat(g: &mut Game, rival: usize) {
    let stats = Arc::make_mut(&mut g.observed_public_empire_stats);
    stats.entry(0).or_default().domestic_tourists = Some(100);
    stats.entry(rival).or_default().foreign_tourists = Some(400);
    stats.entry(rival).or_default().domestic_tourists = Some(10);
}

fn seat_alliance(g: &mut Game, first: usize, second: usize, kind: &str, level: i32) {
    let state = AllianceState {
        kind: kind.to_string(),
        points: match level {
            3 => 240.0,
            2 => 80.0,
            _ => 1.0,
        },
        level,
        ends: g.turn + 30,
    };
    g.players[first].alliances.insert(second, state.clone());
    g.players[second].alliances.insert(first, state);
}

/// The proposals this empire has open right now: from us, not yet expired,
/// and not already answered.
fn proposals_from(g: &Game, pid: usize) -> Vec<(usize, Option<String>, bool)> {
    open_proposals(g, pid)
        .into_iter()
        .map(|deal| (deal.to, deal.alliance.clone(), deal.friendship))
        .collect()
}

fn open_proposals(g: &Game, pid: usize) -> Vec<crate::game::DiplomaticDeal> {
    g.pending_deals
        .iter()
        .filter(|deal| deal.from == pid && deal.expires >= g.turn)
        .cloned()
        .collect()
}

/// Answer an open deal as its recipient, whose turn it is not.
fn answer(g: &mut Game, recipient: usize, deal: u32, accept: bool) {
    let previous = g.current;
    g.current = recipient;
    let action = if accept {
        Action::AcceptDeal { deal }
    } else {
        Action::RejectDeal { deal }
    };
    g.apply(recipient, &action)
        .expect("the deal can be answered");
    g.current = previous;
}

// ---------------------------------------------------------------- registry

/// The gene is registered opt-in, ships off, and its toggles are twins.
#[test]
fn the_gene_is_opt_in_ships_off_and_toggles() {
    let gene = genes::GENES
        .iter()
        .find(|gene| gene.tag == "research-alliance-first")
        .expect("the gene is registered");
    assert_eq!(gene.field, "research_alliance_first");
    assert!(matches!(gene.kind, genes::Kind::OptIn));

    let mut ai = AdvancedAi::new();
    assert!(!ai.research_alliance_first);
    assert!(ai.research_alliance.is_none());
    (gene.enable)(&mut ai);
    assert!(ai.research_alliance_first);
    (gene.disable)(&mut ai);
    assert!(!ai.research_alliance_first);
}

// ------------------------------------------------------------ pure helpers

/// The share is a ratio, and an empire with no science of its own ranks any
/// producing partner ahead of a silent one rather than dividing by zero.
#[test]
fn the_science_share_is_a_ratio_and_survives_a_silent_empire() {
    assert_eq!(science_share(30.0, 60.0), 0.5);
    assert_eq!(science_share(60.0, 60.0), 1.0);
    assert_eq!(science_share(90.0, 60.0), 1.5);
    assert_eq!(science_share(12.0, 0.0), f64::INFINITY);
    assert_eq!(science_share(0.0, 0.0), 0.0);
    assert!(science_share(0.59, 1.0) < ALLY_MIN_SCIENCE_SHARE);
    assert!(science_share(0.6, 1.0) >= ALLY_MIN_SCIENCE_SHARE);
}

/// The premium stops at the top level and at the second route, and is capped
/// inside the opening band.
#[test]
fn the_route_premium_is_capped_in_the_band_and_stops_when_it_buys_nothing() {
    let band = ALLY_ROUTE_OPENING_BAND_CITIES;
    // Inside the band: capped.
    assert_eq!(
        route_premium(1, false, band - 1),
        ALLY_ROUTE_PREMIUM_OPENING_CAP
    );
    assert_eq!(route_premium(1, false, 1), ALLY_ROUTE_PREMIUM_OPENING_CAP);
    // Out of the band: the full premium.
    assert_eq!(route_premium(1, false, band), ALLY_ROUTE_PREMIUM);
    assert_eq!(route_premium(2, false, band + 4), ALLY_ROUTE_PREMIUM);
    // A second route collects no further alliance point.
    assert_eq!(route_premium(1, true, band), 0.0);
    assert_eq!(route_premium(1, true, 1), 0.0);
    // At the science-sharing level a further point buys nothing.
    assert_eq!(route_premium(ALLY_TOP_LEVEL, false, band), 0.0);
    assert_eq!(route_premium(ALLY_TOP_LEVEL + 1, false, band), 0.0);
    // The cap is a cap, not a second premium.
    const { assert!(ALLY_ROUTE_PREMIUM_OPENING_CAP <= ALLY_ROUTE_PREMIUM) };
}

// --------------------------------------------------------------- exclusions

/// A partner below the science floor is no partner, and neither is one at
/// war, unmet, denounced, resented, or without Civil Service.
#[test]
fn the_ranking_excludes_every_infeasible_partner() {
    let mut g = board();
    let ai = ally_ai();
    for other in 1..4 {
        assert!(
            ai.research_alliance_partner_score(&g, 0, other).is_some(),
            "player {other} starts eligible"
        );
    }
    assert!(ai.research_alliance_partner_score(&g, 0, 0).is_none());

    // Below the science floor.
    silence_science(&mut g, 3);
    assert!(ai.research_alliance_partner_score(&g, 0, 3).is_none());

    // At war.
    g.at_war.insert((0, 1));
    assert!(ai.research_alliance_partner_score(&g, 0, 1).is_none());
    g.at_war.clear();

    // Denounced, in either direction.
    g.players[0].denounced_until.insert(1, g.turn + 10);
    assert!(ai.research_alliance_partner_score(&g, 0, 1).is_none());
    g.players[0].denounced_until.clear();
    g.players[1].denounced_until.insert(0, g.turn + 10);
    assert!(ai.research_alliance_partner_score(&g, 0, 1).is_none());
    g.players[1].denounced_until.clear();

    // Grievance at the ceiling.
    g.players[0].grievances.insert(1, ALLY_GRIEVANCE_CEILING);
    assert!(ai.research_alliance_partner_score(&g, 0, 1).is_none());
    g.players[0].grievances.clear();

    // Civil Service is NOT a ranking exclusion: it gates the alliance, not
    // the declared friendship the sequence leads with, and it lands past
    // turn 140 on a Standard clock.
    g.players[1].civics.remove(&crate::name!("civil_service"));
    assert!(ai.research_alliance_partner_score(&g, 0, 1).is_some());
    assert_eq!(ai.research_alliance_kind(&g, 0, 1), None);
    g.players[1].civics.insert(crate::name!("civil_service"));
    g.players[0].civics.remove(&crate::name!("civil_service"));
    assert!(ai.research_alliance_partner_score(&g, 0, 1).is_some());
    assert_eq!(ai.research_alliance_kind(&g, 0, 1), None);
    g.players[0].civics.insert(crate::name!("civil_service"));

    // Not met.
    g.players[0].met.remove(&2);
    assert!(ai.research_alliance_partner_score(&g, 0, 2).is_none());
    g.players[0].met.insert(2);

    // Already allied with us: the objective is met, not repeated.
    seat_alliance(&mut g, 0, 2, "research", 1);
    assert!(ai.research_alliance_partner_score(&g, 0, 2).is_none());
}

/// The guard: neither the culture threat nor the science threat is a partner,
/// and a science threat is recognised from the same public signal the denial
/// layer reads.
#[test]
fn a_culture_or_science_threat_is_never_a_partner() {
    let mut g = board();
    let ai = ally_ai();

    make_culture_threat(&mut g, 1);
    assert!(ai.culture_trade_threats(&g, 0).contains(&1));
    assert!(ai.research_alliance_excluded(&g, 0, 1));
    assert!(ai.research_alliance_partner_score(&g, 0, 1).is_none());
    assert_ne!(ai.research_alliance_partner(&g, 0), Some(1));

    // A science threat: the rival's leading lane is Science and it has passed
    // the bar. The launch ladder is the public evidence of it.
    let mut g = board();
    assert!(!ai.research_alliance_science_threat(&g, 2));
    g.players[2]
        .science_projects
        .insert("launch_moon_landing".to_string());
    assert!(ai.research_alliance_science_threat(&g, 2));
    assert!(ai.research_alliance_excluded(&g, 0, 2));
    assert!(ai.research_alliance_partner_score(&g, 0, 2).is_none());

    // The bar is a bar: the first satellite alone does not clear it.
    let mut g = board();
    g.players[2]
        .science_projects
        .insert("launch_earth_satellite".to_string());
    assert!(!ai.research_alliance_science_threat(&g, 2));
    assert!(ai.research_alliance_partner_score(&g, 0, 2).is_some());

    // With the science lane disabled there is no science threat to read, and
    // the same rival becomes an ordinary candidate again.
    let mut g = board();
    g.players[2]
        .science_projects
        .insert("launch_moon_landing".to_string());
    g.victory_conditions.science = false;
    assert!(!ai.research_alliance_science_threat(&g, 2));
    assert!(ai.research_alliance_partner_score(&g, 0, 2).is_some());
}

/// Ranking prefers the larger science, and a declared friendship already in
/// place outranks a merely bigger rival.
#[test]
fn the_ranking_prefers_science_and_a_friendship_already_in_place() {
    let mut g = board();
    let ai = ally_ai();
    // Two equal partners: the tie is broken by the lower id, so 1 leads.
    assert_eq!(ai.research_alliance_partner(&g, 0), Some(1));

    // Give 2 a second city: more science, so 2 leads.
    let seat = g
        .map
        .tiles
        .keys()
        .copied()
        .filter(|position| {
            g.rules.is_passable(&g.map.tiles[position])
                && !g.rules.is_water(&g.map.tiles[position])
                && g.map.tiles[position].owner_city.is_none()
                && g.city_at(*position).is_none()
        })
        .max_by_key(|position| {
            (
                g.wdist(*position, g.cities[&g.player_city_ids(2)[0]].pos),
                *position,
            )
        })
        .expect("open land somewhere");
    g.found_city_for(2, seat, None);
    assert!(
        AdvancedAi::research_alliance_science(&g, 2) > AdvancedAi::research_alliance_science(&g, 1)
    );
    assert_eq!(ai.research_alliance_partner(&g, 0), Some(2));

    // A declared friendship with the smaller partner is worth more than the
    // extra city: the friendship is the prerequisite an alliance needs.
    g.players[0].friends_until.insert(1, g.turn + 20);
    g.players[1].friends_until.insert(0, g.turn + 20);
    assert!(g.are_friends(0, 1));
    assert_eq!(ai.research_alliance_partner(&g, 0), Some(1));
}

// ----------------------------------------------------------------- cadence

/// Without Civil Service the friendship is still proposed and the alliance
/// is not: the sequence waits with the friendship in hand rather than
/// spending turns on a proposal the engine would refuse.
#[test]
fn the_friendship_leads_and_waits_for_civil_service() {
    let mut g = board();
    let mut ai = ally_ai();
    for pid in 0..4 {
        g.players[pid].civics.remove(&crate::name!("civil_service"));
    }
    ai.research_alliance_step(&mut g, 0);
    assert_eq!(proposals_from(&g, 0), vec![(1, None, true)]);

    let deal = open_proposals(&g, 0)[0].id;
    answer(&mut g, 1, deal, true);
    assert!(g.are_friends(0, 1));

    // The friendship stands, Civil Service does not: no further proposal.
    for turn in 61..80 {
        g.turn = turn;
        ai.research_alliance_step(&mut g, 0);
        assert!(proposals_from(&g, 0).is_empty(), "waiting at turn {turn}");
    }
    // Both trees adopt it: the alliance follows at once.
    g.players[0].civics.insert(crate::name!("civil_service"));
    g.players[1].civics.insert(crate::name!("civil_service"));
    ai.research_alliance_step(&mut g, 0);
    assert_eq!(
        proposals_from(&g, 0),
        vec![(1, Some("research".to_string()), true)]
    );
}

/// The friendship comes first, the alliance once it stands, one proposal a
/// turn, and a partner asked is not asked again inside the cool-down.
#[test]
fn the_sequence_is_friendship_then_alliance_with_a_cool_down() {
    let mut g = board();
    let mut ai = ally_ai();

    ai.research_alliance_step(&mut g, 0);
    let asked = proposals_from(&g, 0);
    assert_eq!(asked.len(), 1, "one proposal a turn");
    let (partner, alliance, friendship) = asked[0].clone();
    assert_eq!(partner, 1);
    assert_eq!(alliance, None, "the friendship comes first");
    assert!(friendship);
    assert_eq!(
        g.players[0]
            .counters
            .get("research_alliance:friendships_proposed"),
        Some(&1)
    );
    assert_eq!(
        ai.research_alliance.as_ref().unwrap().asked.get(&1),
        Some(&60)
    );

    // While that answer is outstanding nobody is asked — not the same
    // partner, and not the next-best empire either.
    g.turn += 1;
    ai.research_alliance_step(&mut g, 0);
    assert_eq!(proposals_from(&g, 0).len(), 1);

    // The friendship is accepted; the alliance follows on the next ask.
    let deal = open_proposals(&g, 0)[0].id;
    answer(&mut g, 1, deal, true);
    assert!(g.are_friends(0, 1));
    g.turn = 60 + ALLY_RETRY_TURNS;
    ai.research_alliance_step(&mut g, 0);
    let asked = proposals_from(&g, 0);
    assert!(
        asked.contains(&(1, Some("research".to_string()), true)),
        "the alliance follows the friendship: {asked:?}"
    );
    // Once the alliance stands the desk stops proposing altogether.
    seat_alliance(&mut g, 0, 1, "research", 1);
    let before = g.pending_deals.len();
    g.turn += ALLY_RETRY_TURNS;
    ai.research_alliance_step(&mut g, 0);
    assert_eq!(g.pending_deals.len(), before, "the objective is met");
    assert_eq!(
        g.players[0]
            .counters
            .get("research_alliance:alliances_proposed"),
        Some(&1)
    );
}

/// A partner is not asked again before the cool-down expires, even with the
/// earlier proposal gone.
#[test]
fn a_refused_partner_waits_out_the_cool_down() {
    let mut g = board();
    let mut ai = ally_ai();
    // Only one candidate, so the cool-down is the whole answer.
    silence_science(&mut g, 2);
    silence_science(&mut g, 3);
    ai.research_alliance_step(&mut g, 0);
    assert_eq!(proposals_from(&g, 0), vec![(1, None, true)]);
    let deal = open_proposals(&g, 0)[0].id;
    answer(&mut g, 1, deal, false);
    assert!(proposals_from(&g, 0).is_empty());

    for turn in 61..60 + ALLY_RETRY_TURNS {
        g.turn = turn;
        ai.research_alliance_step(&mut g, 0);
        assert!(
            proposals_from(&g, 0).is_empty(),
            "still inside the cool-down at turn {turn}"
        );
    }
    g.turn = 60 + ALLY_RETRY_TURNS;
    ai.research_alliance_step(&mut g, 0);
    assert_eq!(
        proposals_from(&g, 0),
        vec![(1, None, true)],
        "a refusal is answered by the cool-down, not by moving on"
    );

    // The sequence moves on only when the partner stops being a candidate:
    // a war releases them and the next-best empire takes the place, with no
    // canvassing in between.
    let mut moved_on = board();
    let mut moved_on_ai = ally_ai();
    silence_science(&mut moved_on, 3);
    moved_on_ai.research_alliance_step(&mut moved_on, 0);
    assert_eq!(proposals_from(&moved_on, 0), vec![(1, None, true)]);
    let deal = open_proposals(&moved_on, 0)[0].id;
    answer(&mut moved_on, 1, deal, false);
    // One turn later, with the partner still a candidate, nobody is asked.
    moved_on.turn = 61;
    moved_on_ai.research_alliance_step(&mut moved_on, 0);
    assert!(proposals_from(&moved_on, 0).is_empty());
    // War releases them; the next-best empire is asked at once.
    moved_on.at_war.insert((0, 1));
    moved_on_ai.research_alliance_step(&mut moved_on, 0);
    assert_eq!(
        proposals_from(&moved_on, 0),
        vec![(2, None, true)],
        "a released partner's place goes to the next-best empire"
    );
}

/// A Research Alliance already taken on either side falls back to the free
/// kind whose model yield is closest to science, never to nothing.
#[test]
fn an_unavailable_research_alliance_falls_back_by_model_yield() {
    let mut g = board();
    let ai = ally_ai();
    assert_eq!(ai.research_alliance_kind(&g, 0, 1), Some("research"));

    // Our own research slot is taken by somebody else.
    seat_alliance(&mut g, 0, 3, "research", 1);
    assert_eq!(ai.research_alliance_kind(&g, 0, 1), Some("cultural"));

    // Cultural too: economic is next, then religious, then military.
    seat_alliance(&mut g, 0, 2, "cultural", 1);
    assert_eq!(ai.research_alliance_kind(&g, 0, 1), Some("economic"));
    seat_alliance(&mut g, 1, 2, "economic", 1);
    assert_eq!(ai.research_alliance_kind(&g, 0, 1), Some("religious"));
    seat_alliance(&mut g, 1, 3, "religious", 1);
    assert_eq!(ai.research_alliance_kind(&g, 0, 1), Some("military"));

    // Research is unavailable for want of the tech, not the slot.
    let mut g = board();
    g.players[1]
        .techs
        .remove(&crate::name!("scientific_theory"));
    assert_eq!(ai.research_alliance_kind(&g, 0, 1), Some("cultural"));
    let mut g = board();
    g.players[0]
        .techs
        .remove(&crate::name!("scientific_theory"));
    assert_eq!(ai.research_alliance_kind(&g, 0, 1), Some("cultural"));
}

/// A partner who becomes a threat loses the objective — the desk forgets the
/// ask — and the standing alliance is left alone.
#[test]
fn the_objective_is_dropped_but_the_alliance_is_never_broken() {
    let mut g = board();
    let mut ai = ally_ai();
    ai.research_alliance_step(&mut g, 0);
    assert!(ai
        .research_alliance
        .as_ref()
        .unwrap()
        .asked
        .contains_key(&1));

    seat_alliance(&mut g, 0, 1, "research", 2);
    make_culture_threat(&mut g, 1);
    g.turn += 1;
    ai.research_alliance_step(&mut g, 0);
    assert!(
        !ai.research_alliance
            .as_ref()
            .unwrap()
            .asked
            .contains_key(&1),
        "the objective is dropped"
    );
    assert!(
        g.alliance_with(0, 1).is_some(),
        "the alliance itself is never broken"
    );
    assert_eq!(ai.research_alliance_route_premium(&g, 0, 1), 0.0);
}

// ------------------------------------------------------------ routes, cards

/// The route premium reaches the real valuation, is zero without an alliance,
/// and is zero with the gene off.
#[test]
fn the_route_premium_reaches_the_valuation_and_is_zero_off() {
    let mut g = board();
    let ai = ally_ai();
    assert_eq!(ai.research_alliance_route_premium(&g, 0, 1), 0.0);

    seat_alliance(&mut g, 0, 1, "research", 1);
    // One city: inside the opening band, so the cap applies.
    assert_eq!(
        ai.research_alliance_route_premium(&g, 0, 1),
        ALLY_ROUTE_PREMIUM_OPENING_CAP
    );
    assert_eq!(stock_ai().research_alliance_route_premium(&g, 0, 1), 0.0);

    // The valuation itself moves by exactly the premium.
    let city = g.cities[&g.player_city_ids(1)[0]].clone();
    let on = ai.trade_route_destination_value(&g, 0, &city, GrandStrategy::Science);
    let off = stock_ai().trade_route_destination_value(&g, 0, &city, GrandStrategy::Science);
    assert!((on - off - ALLY_ROUTE_PREMIUM_OPENING_CAP).abs() < 1e-9);

    // Our own city is nobody's ally and carries nothing.
    let own = g.cities[&g.player_city_ids(0)[0]].clone();
    let on = ai.trade_route_destination_value(&g, 0, &own, GrandStrategy::Science);
    let off = stock_ai().trade_route_destination_value(&g, 0, &own, GrandStrategy::Science);
    assert_eq!(on, off);
}

/// The card is wanted only while such an alliance stands, only when offered,
/// and never with the gene off.
#[test]
fn the_international_route_card_is_wanted_only_while_the_alliance_stands() {
    let mut g = board();
    let ai = ally_ai();
    assert!(ai.research_alliance_cards(&g, 0).is_empty());

    seat_alliance(&mut g, 0, 1, "research", 1);
    // The cards are era-gated; whatever is offered, at most one is named and
    // it is the best offered one in table order.
    let cards = ai.research_alliance_cards(&g, 0);
    assert!(cards.len() <= 1);
    for card in &cards {
        assert!(ALLY_ROUTE_CARDS.contains(card));
        assert!(g.rules.policies[*card].offered(&g.players[0].age, g.world_era));
        // The claim the card is chosen for: science on an international route.
        assert!(
            g.rules.policies[*card]
                .effects
                .get("international_trade_science")
                .copied()
                .unwrap_or(0.0)
                > 0.0
        );
    }
    // Offered late: at the world era where both are on the table the best one
    // is Market Economy, which pays twice Trade Confederation's science.
    assert!(
        g.rules.policies["market_economy"]
            .effects
            .get("international_trade_science")
            > g.rules.policies["trade_confederation"]
                .effects
                .get("international_trade_science")
    );
    assert_eq!(ALLY_ROUTE_CARDS[0], "market_economy");

    assert!(stock_ai().research_alliance_cards(&g, 0).is_empty());

    // A threat ally justifies no card.
    make_culture_threat(&mut g, 1);
    assert!(ai.research_alliance_cards(&g, 0).is_empty());
}

/// Off, nothing this gene owns does anything at all.
#[test]
fn off_the_gene_is_inert() {
    let mut g = board();
    let mut stock = stock_ai();
    seat_alliance(&mut g, 0, 1, "research", 1);
    stock.research_alliance_step(&mut g, 0);
    assert!(g.pending_deals.is_empty());
    assert!(stock.research_alliance.is_none());
    assert!(stock.research_alliance_cards(&g, 0).is_empty());
    assert_eq!(stock.research_alliance_route_premium(&g, 0, 1), 0.0);
    assert!(g.players[0]
        .counters
        .keys()
        .all(|key| !key.starts_with("research_alliance:")));
}

/// ★ A UNIT TEST THAT PROPOSES ON A HAND-BUILT BOARD PROVES ONLY THAT THE
/// BOARD WAS BUILT RIGHT. This plays a whole game at the screen's own size
/// with the gene on and requires the desk to have actually asked somebody:
/// the preconditions it waits for — a met major inside the science floor, no
/// denouncement, and for the alliance itself Civil Service on two trees —
/// all have to arrive on their own. Written after the first version of this
/// gene measured a flat probe: at 4 majors on a 44x30 board over 160 turns
/// nobody ever reaches Civil Service, so the desk never fired at all and the
/// screen was pricing seat assignment.
#[test]
fn the_desk_reaches_a_real_game_and_asks() {
    let mut world = Game::new_full(6, 74, 46, 26_081_900, 250, 9, true);
    let mut ais: Vec<AdvancedAi> = (0..world.players.len())
        .map(|pid| {
            if world.players[pid].is_minor || world.players[pid].is_barbarian {
                AdvancedAi::new()
            } else {
                ally_ai()
            }
        })
        .collect();
    crate::ai::run_game(&mut world, &mut ais);
    let asked: i64 = world
        .players
        .iter()
        .flat_map(|player| {
            [
                "research_alliance:friendships_proposed",
                "research_alliance:alliances_proposed",
            ]
            .into_iter()
            .filter_map(|key| player.counters.get(key).copied())
        })
        .sum();
    assert!(
        asked > 0,
        "the desk never asked anybody in a whole game: the gene cannot be \
         priced by a screen it does not reach"
    );
}
