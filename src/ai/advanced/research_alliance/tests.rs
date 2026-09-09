use super::*;
use crate::ai::advanced::{genes, GrandStrategy, StrategicPlan, VictoryTarget};
use crate::game::{Action, AllianceState};
use std::sync::Arc;

/// Four majors, one city each, every pair met and at peace, Civil Service and
/// Scientific Theory on every tree so a Research Alliance is legal between
/// any two of them. Turn 60 is our cadence turn (`60 % 12 == 0`), so the
/// stock desk would ask on it; turn 61 is not.
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

fn plan(strategy: GrandStrategy, turn: u32) -> StrategicPlan {
    StrategicPlan {
        strategy,
        target_player: None,
        target_city: None,
        threatened_city: None,
        desired_cities: 4,
        assessed_turn: turn,
        rush: false,
    }
}

/// Strip an empire down to no cities, so its science is zero.
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

/// The proposals this empire has open right now: `(to, alliance kind)`.
fn proposals_from(g: &Game, pid: usize) -> Vec<(usize, Option<String>)> {
    g.pending_deals
        .iter()
        .filter(|deal| deal.from == pid && deal.expires >= g.turn)
        .map(|deal| (deal.to, deal.alliance.clone()))
        .collect()
}

/// Answer every open proposal from `pid` as its recipient.
fn answer_all(g: &mut Game, pid: usize, accept: bool) {
    let deals: Vec<(u32, usize)> = g
        .pending_deals
        .iter()
        .filter(|deal| deal.from == pid)
        .map(|deal| (deal.id, deal.to))
        .collect();
    for (deal, recipient) in deals {
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
    assert!(ai.research_alliance_asked.is_empty());
    (gene.enable)(&mut ai);
    assert!(ai.research_alliance_first);
    (gene.disable)(&mut ai);
    assert!(!ai.research_alliance_first);
}

// ------------------------------------------------------------ pure helpers

/// The share is a ratio, clamped to the weight so an empire with no science
/// of its own still produces a finite, comparable score.
#[test]
fn the_science_share_is_a_clamped_ratio() {
    assert_eq!(science_share(30.0, 60.0), 0.5);
    assert_eq!(science_share(60.0, 60.0), 1.0);
    assert_eq!(science_share(90.0, 60.0), 1.5);
    assert_eq!(science_share(12.0, 0.0), ALLY_SCIENCE_WEIGHT);
    assert_eq!(science_share(0.0, 0.0), 0.0);
    assert_eq!(science_share(1.0e9, 1.0), ALLY_SCIENCE_WEIGHT);
}

/// The premium stops at the top level and at the second route, and is capped
/// inside the opening band.
#[test]
fn the_route_premium_is_capped_in_the_band_and_stops_when_it_buys_nothing() {
    let band = ALLY_ROUTE_OPENING_BAND_CITIES;
    assert_eq!(
        route_premium(1, false, band - 1),
        ALLY_ROUTE_PREMIUM_OPENING_CAP
    );
    assert_eq!(route_premium(1, false, 1), ALLY_ROUTE_PREMIUM_OPENING_CAP);
    assert_eq!(route_premium(1, false, band), ALLY_ROUTE_PREMIUM);
    assert_eq!(route_premium(2, false, band + 4), ALLY_ROUTE_PREMIUM);
    assert_eq!(route_premium(1, true, band), 0.0);
    assert_eq!(route_premium(1, true, 1), 0.0);
    assert_eq!(route_premium(ALLY_TOP_LEVEL, false, band), 0.0);
    assert_eq!(route_premium(ALLY_TOP_LEVEL + 1, false, band), 0.0);
    const { assert!(ALLY_ROUTE_PREMIUM_OPENING_CAP <= ALLY_ROUTE_PREMIUM) };
}

/// Level 2 needs `80 / 1.5` turns, rounded up: 54. On a 250-turn clock an
/// alliance seated on turn 196 can still reach it; one seated on 197 cannot.
#[test]
fn level_two_is_reachable_until_fifty_four_turns_remain() {
    assert!(level_two_reachable(1, 250));
    assert!(level_two_reachable(196, 250));
    assert!(!level_two_reachable(197, 250));
    assert!(!level_two_reachable(250, 250));
    assert!(!level_two_reachable(300, 250));
}

// -------------------------------------------------------------------- lane

/// Off the lane is `Stock`; on it is `Research` when legal on our tree,
/// `Wait` when not, and `Stock` again once the alliance is held or level 2
/// is out of reach.
#[test]
fn the_lane_is_research_wait_or_stock() {
    let mut g = board();
    assert_eq!(
        stock_ai().research_alliance_lane(&g, 0),
        ResearchAllianceLane::Stock
    );
    assert_eq!(
        ally_ai().research_alliance_lane(&g, 0),
        ResearchAllianceLane::Research
    );

    // Our own tree short of Scientific Theory: wait, propose nothing else.
    g.players[0]
        .techs
        .remove(&crate::name!("scientific_theory"));
    assert_eq!(
        ally_ai().research_alliance_lane(&g, 0),
        ResearchAllianceLane::Wait
    );
    // ... unless level 2 can no longer be reached before the clock ends.
    g.turn = 200;
    assert_eq!(
        ally_ai().research_alliance_lane(&g, 0),
        ResearchAllianceLane::Stock
    );

    // Held: the desk is the stock desk again.
    let mut g = board();
    seat_alliance(&mut g, 0, 1, ALLY_RESEARCH_KIND, 1);
    assert_eq!(
        ally_ai().research_alliance_lane(&g, 0),
        ResearchAllianceLane::Stock
    );
}

// -------------------------------------------------------------------- desk

/// On a turn the stock cadence would skip, the gene asks for a Research
/// Alliance regardless of the plan's own kind; off, nothing is asked.
#[test]
fn the_desk_asks_for_research_off_cadence_whatever_the_plan_says() {
    let mut g = board();
    g.turn = 61;
    let plan = plan(GrandStrategy::Expansion, g.turn);

    let mut off = stock_ai();
    off.propose_strategic_alliance(&mut g, 0, &plan, None);
    assert!(
        proposals_from(&g, 0).is_empty(),
        "off, 61 is not a cadence turn"
    );

    let mut on = ally_ai();
    on.propose_strategic_alliance(&mut g, 0, &plan, None);
    let proposals = proposals_from(&g, 0);
    assert_eq!(proposals.len(), 1);
    assert_eq!(proposals[0].1.as_deref(), Some(ALLY_RESEARCH_KIND));
    assert_eq!(
        g.players[0]
            .counters
            .get("research_alliance:alliances_proposed"),
        Some(&1)
    );
    assert_eq!(on.research_alliance_asked.get(&proposals[0].0), Some(&61));
}

/// While our tree lacks Scientific Theory the gene proposes no other kind on
/// the cadence turn where the stock desk would seat an economic alliance —
/// and resumes the stock kind once level 2 is out of reach.
#[test]
fn the_desk_waits_for_scientific_theory_and_holds_the_slot() {
    let mut g = board();
    g.players[0]
        .techs
        .remove(&crate::name!("scientific_theory"));
    let plan = plan(GrandStrategy::Expansion, g.turn);

    let mut off = stock_ai();
    off.propose_strategic_alliance(&mut g, 0, &plan, None);
    let stock = proposals_from(&g, 0);
    assert_eq!(stock.len(), 1);
    assert_eq!(stock[0].1.as_deref(), Some("economic"));
    g.pending_deals.clear();

    let mut on = ally_ai();
    on.propose_strategic_alliance(&mut g, 0, &plan, None);
    assert!(
        proposals_from(&g, 0).is_empty(),
        "the slot is held for the Research Alliance"
    );

    // Turn 204 is a cadence turn past the level-2 horizon: the stock kind.
    g.turn = 204;
    let plan = super::super::StrategicPlan {
        assessed_turn: g.turn,
        ..plan
    };
    on.propose_strategic_alliance(&mut g, 0, &plan, None);
    let late = proposals_from(&g, 0);
    assert_eq!(late.len(), 1);
    assert_eq!(late[0].1.as_deref(), Some("economic"));
}

/// The science term picks the partner with the science; the stock ranking
/// alone breaks the tie by the lowest id.
#[test]
fn the_ranking_prefers_the_partner_with_the_science() {
    let mut g = board();
    silence_science(&mut g, 1);
    silence_science(&mut g, 3);
    let plan = plan(GrandStrategy::Science, g.turn);

    let mut off = stock_ai();
    off.propose_strategic_alliance(&mut g, 0, &plan, None);
    assert_eq!(
        proposals_from(&g, 0)[0].0,
        1,
        "stock: the tie goes to the lowest id"
    );
    g.pending_deals.clear();

    let mut on = ally_ai();
    on.propose_strategic_alliance(&mut g, 0, &plan, None);
    assert_eq!(
        proposals_from(&g, 0)[0].0,
        2,
        "on: the partner with the science"
    );
}

/// A refusal is answered by the cool-down: the next-best partner is asked
/// in the meantime, and the refuser again once it expires.
#[test]
fn a_refused_partner_waits_out_the_cool_down() {
    let mut g = board();
    // Two candidates only: the third is resented past the stock ceiling.
    g.players[0].grievances.insert(3, 80.0);
    let mut ai = ally_ai();
    let science = plan(GrandStrategy::Science, g.turn);

    ai.propose_strategic_alliance(&mut g, 0, &science, None);
    let first = proposals_from(&g, 0)[0].0;
    answer_all(&mut g, 0, false);

    g.turn += 1;
    ai.propose_strategic_alliance(&mut g, 0, &science, None);
    let second = proposals_from(&g, 0)[0].0;
    assert_ne!(second, first, "the refuser is inside the cool-down");
    answer_all(&mut g, 0, false);

    g.turn += 1;
    ai.propose_strategic_alliance(&mut g, 0, &science, None);
    assert!(
        proposals_from(&g, 0).is_empty(),
        "both candidates are cooling down and the third is no partner"
    );

    g.turn = 60 + ALLY_RETRY_TURNS;
    ai.propose_strategic_alliance(&mut g, 0, &science, None);
    assert_eq!(proposals_from(&g, 0)[0].0, first, "the cool-down expired");
}

/// A culture threat is barred, and the stock desk's denied partner stays
/// denied under the gene.
#[test]
fn a_culture_threat_and_the_denied_partner_are_never_asked() {
    let mut g = board();
    silence_science(&mut g, 1);
    silence_science(&mut g, 3);
    let science = plan(GrandStrategy::Science, g.turn);

    let mut on = ally_ai();
    on.propose_strategic_alliance(&mut g, 0, &science, Some(2));
    assert!(proposals_from(&g, 0).iter().all(|(to, _)| *to != 2));
    g.pending_deals.clear();

    let mut g = board();
    silence_science(&mut g, 1);
    silence_science(&mut g, 3);
    make_culture_threat(&mut g, 2);
    let mut on = ally_ai();
    assert!(on.research_alliance_barred(&g, 0, 2));
    assert!(!stock_ai().research_alliance_barred(&g, 0, 2));
    on.propose_strategic_alliance(&mut g, 0, &science, None);
    assert!(proposals_from(&g, 0).iter().all(|(to, _)| *to != 2));
}

/// Once a Research Alliance stands the desk is the stock desk: off cadence
/// it asks nothing.
#[test]
fn a_held_research_alliance_ends_the_gene_s_asking() {
    let mut g = board();
    seat_alliance(&mut g, 0, 1, ALLY_RESEARCH_KIND, 1);
    g.turn = 61;
    let mut ai = ally_ai();
    let expansion = plan(GrandStrategy::Expansion, g.turn);
    ai.propose_strategic_alliance(&mut g, 0, &expansion, None);
    assert!(proposals_from(&g, 0).is_empty());
}

// ------------------------------------------------------------------ routes

/// The premium reaches the route valuation for a research ally only, and is
/// zero off.
#[test]
fn the_route_premium_reaches_the_valuation_for_a_research_ally_only() {
    let mut g = board();
    let ai = ally_ai();
    assert_eq!(ai.research_alliance_route_premium(&g, 0, 1), 0.0);

    seat_alliance(&mut g, 0, 2, "cultural", 1);
    assert_eq!(ai.research_alliance_route_premium(&g, 0, 2), 0.0);

    seat_alliance(&mut g, 0, 1, ALLY_RESEARCH_KIND, 1);
    // One city: inside the opening band, so the cap applies.
    assert_eq!(
        ai.research_alliance_route_premium(&g, 0, 1),
        ALLY_ROUTE_PREMIUM_OPENING_CAP
    );
    assert_eq!(stock_ai().research_alliance_route_premium(&g, 0, 1), 0.0);

    let city = g.cities[&g.player_city_ids(1)[0]].clone();
    let on = ai.trade_route_destination_value(&g, 0, &city, GrandStrategy::Science);
    let off = stock_ai().trade_route_destination_value(&g, 0, &city, GrandStrategy::Science);
    assert!((on - off - ALLY_ROUTE_PREMIUM_OPENING_CAP).abs() < 1e-9);

    let own = g.cities[&g.player_city_ids(0)[0]].clone();
    let on = ai.trade_route_destination_value(&g, 0, &own, GrandStrategy::Science);
    let off = stock_ai().trade_route_destination_value(&g, 0, &own, GrandStrategy::Science);
    assert_eq!(on, off);
}

// --------------------------------------------------------------------- off

/// Off, every hook the stock desk and the route valuation call returns the
/// identity: the lane is `Stock`, nobody is barred, the science term and the
/// premium are zero, and an ask is not recorded.
#[test]
fn off_the_gene_is_inert() {
    let mut g = board();
    seat_alliance(&mut g, 0, 1, ALLY_RESEARCH_KIND, 1);
    let mut ai = stock_ai();
    assert_eq!(
        ai.research_alliance_lane(&g, 0),
        ResearchAllianceLane::Stock
    );
    assert!(!ai.research_alliance_barred(&g, 0, 2));
    assert_eq!(
        ai.research_alliance_science_term(&g, 0, 2, ALLY_RESEARCH_KIND),
        0.0
    );
    assert_eq!(ai.research_alliance_route_premium(&g, 0, 1), 0.0);
    ai.research_alliance_note_asked(&mut g, 0, 2, ALLY_RESEARCH_KIND);
    assert!(ai.research_alliance_asked.is_empty());
    assert!(!g.players[0]
        .counters
        .contains_key("research_alliance:alliances_proposed"));
}

// ------------------------------------------------------------- instrument

/// The whole-game instrument, on the screen's own setup (`gene_screen
/// --difficulty emperor --handicap rivals --rival-chairs 3`): 6 majors on
/// 74×46 for 250 turns, seats 0-2 measured with the gene on and exempt from
/// the rung's handicap, seats 3-5 stock rivals playing Emperor's bonuses.
/// Reports, per major, the turn Civil Service and Scientific Theory landed,
/// the alliances held at the end, and the desk's counter. Not an assertion —
/// whether a Research Alliance is reached on this clock is what it measures.
///
/// ```text
/// cargo test --profile ci --lib research_alliance_whole_game_instrument -- --ignored --nocapture
/// ```
#[test]
#[ignore]
fn research_alliance_whole_game_instrument() {
    use crate::game::GameOptions;
    for seed in [26_081_900_u64, 26_081_901, 26_081_902] {
        let mut world = Game::new_with(GameOptions {
            randomize_civs: true,
            difficulty: "emperor".to_string(),
            handicap_exempt: (0..3).collect(),
            ..GameOptions::new(6, 74, 46, seed, 250, 9)
        });
        let mut ais: Vec<AdvancedAi> = (0..world.players.len())
            .map(|pid| {
                if world.players[pid].is_minor || world.players[pid].is_barbarian || pid >= 3 {
                    AdvancedAi::new()
                } else {
                    ally_ai()
                }
            })
            .collect();
        let mut civil_service = [None; 6];
        let mut scientific_theory = [None; 6];
        let mut first_research_alliance = [None; 6];
        crate::ai::run_game_observed(&mut world, &mut ais, |g| {
            for pid in 0..6 {
                if civil_service[pid].is_none()
                    && g.players[pid]
                        .civics
                        .contains(&crate::name!("civil_service"))
                {
                    civil_service[pid] = Some(g.turn);
                }
                if scientific_theory[pid].is_none()
                    && g.players[pid]
                        .techs
                        .contains(&crate::name!("scientific_theory"))
                {
                    scientific_theory[pid] = Some(g.turn);
                }
                if first_research_alliance[pid].is_none()
                    && g.players[pid]
                        .alliances
                        .values()
                        .any(|alliance| alliance.kind == ALLY_RESEARCH_KIND)
                {
                    first_research_alliance[pid] = Some(g.turn);
                }
            }
        });
        for pid in 0..6 {
            let p = &world.players[pid];
            let alliances: Vec<String> = p
                .alliances
                .iter()
                .map(|(o, a)| format!("{o}:{}L{}", a.kind, a.level))
                .collect();
            println!(
                "seed {seed} pid {pid} {} on={} civil_service={:?} scientific_theory={:?} \
                 research_alliance_from={:?} alliances={:?} asked={:?} score={} turn={}",
                p.civ,
                pid < 3,
                civil_service[pid],
                scientific_theory[pid],
                first_research_alliance[pid],
                alliances,
                p.counters.get("research_alliance:alliances_proposed"),
                world.score(pid),
                world.turn
            );
        }
    }
}
