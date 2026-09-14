use super::*;
use crate::ai::advanced::test_support::opt_in_off_in_both_controllers;
use crate::ai::advanced::{GrandStrategy, VictoryTarget};
use crate::game::DealItems;

/// Three majors, one city each, all at peace and mutually known. Seat 0 is
/// the denier; seat 1 is the rival that will race; seat 2 is the bystander.
fn board() -> Game {
    let mut g = Game::new_full(3, 30, 20, 79_208, 250, 0, false);
    for pid in 0..3 {
        let settler = g
            .player_unit_ids(pid)
            .into_iter()
            .find(|uid| g.units[uid].kind == "settler")
            .expect("each fixture major begins with a Settler");
        let position = g.units[&settler].pos;
        g.found_city_for(pid, position, None);
        for uid in g.player_unit_ids(pid) {
            g.remove_unit(uid);
        }
        g.players[pid].gold = 1_000.0;
        g.players[pid].gold_per_turn = 10.0;
    }
    g.current = 0;
    g.turn = 60;
    g.at_war.clear();
    g.record_contact(0, 1);
    g.record_contact(0, 2);
    g
}

/// The neutral plan the unit ladder passes down: no threatened city, so the
/// raid's home-defence guard is not the thing under test.
fn plan() -> StrategicPlan {
    StrategicPlan {
        strategy: GrandStrategy::Science,
        target_player: None,
        target_city: None,
        threatened_city: None,
        desired_cities: 4,
        assessed_turn: 0,
        rush: false,
    }
}

/// Put seats `a` and `b` at war, the way the fixtures in `advanced/tests.rs`
/// do.
fn declare_war(g: &mut Game, a: usize, b: usize) {
    g.at_war.insert((a, b));
    g.at_war.insert((b, a));
}

/// Rungs 1 and 2: `science-threat-denial` alone.
fn denier() -> AdvancedAi {
    let mut ai = AdvancedAi::targeting(VictoryTarget::Science);
    ai.enable_science_threat_denial();
    ai
}

/// Rungs 1 to 4: `science-denial-war`, which arms the base.
fn warrior() -> AdvancedAi {
    let mut ai = AdvancedAi::targeting(VictoryTarget::Science);
    ai.enable_science_denial_war();
    ai
}

/// Give `owner` a Spaceport on an owned tile of its first city and let seat 0
/// see the tile. Returns the city and the pad.
fn give_pad(g: &mut Game, owner: usize) -> (u32, Pos) {
    let cid = g.player_city_ids(owner)[0];
    let centre = g.cities[&cid].pos;
    let pad = g.cities[&cid]
        .owned_tiles
        .iter()
        .copied()
        .find(|pos| {
            *pos != centre
                && g.map
                    .get(*pos)
                    .is_some_and(|tile| tile.district.is_none() && !g.rules.is_water(tile))
        })
        .expect("the fixture city owns a free land tile");
    let tile = g.map.tiles.get_mut(&pad).unwrap();
    tile.owner_city = Some(cid);
    tile.district = Some(crate::name!("spaceport"));
    tile.pillaged = false;
    tile.improvement = None;
    g.cities
        .get_mut(&cid)
        .unwrap()
        .districts
        .insert(crate::name!("spaceport"), pad);
    g.players[0].explored.insert(pad);
    (cid, pad)
}

/// Hand `owner` `count` technologies seat 0 does not have.
fn give_tech_lead(g: &mut Game, owner: usize, count: usize) {
    let ours = g.players[0].techs.clone();
    let extra: Vec<crate::name::Name> = g
        .rules
        .techs
        .keys()
        .map(|tech| crate::name::Name::new(tech.as_str()))
        .filter(|tech| !ours.contains(tech))
        .take(count)
        .collect();
    assert_eq!(extra.len(), count, "the ruleset has enough technologies");
    for tech in extra {
        g.players[owner].techs.insert(tech);
    }
}

/// A passage sale to `partner`, the archetypal deal the veto refuses.
fn passage_sale(partner: usize) -> QuickDeal {
    QuickDeal {
        partner,
        category: "diplomatic".into(),
        item: "open_borders".into(),
        direction: "sell".into(),
        offer: DealItems::default(),
        request: DealItems::default(),
        my_value: 100.0,
        partner_value: 100.0,
    }
}

#[test]
fn the_gene_is_opt_in_and_ships_off_in_both_controllers() {
    opt_in_off_in_both_controllers("science-threat-denial", |ai| ai.science_threat_denial);
    opt_in_off_in_both_controllers("science-denial-war", |ai| ai.science_denial_war);
}

/// The war tag requires the base and arms it, so a seat that drew the war
/// without the base plays the whole ladder; turning the war off leaves the
/// base as it was, and turning the base off leaves the war inert.
#[test]
fn the_war_tag_arms_the_base_and_is_inert_without_it() {
    let mut ai = AdvancedAi::new();
    ai.enable_science_denial_war();
    assert!(ai.science_threat_denial && ai.science_denial_war);
    assert!(ai.science_denial_war_active());
    ai.disable_science_denial_war();
    assert!(ai.science_threat_denial, "the base stays on");
    assert!(!ai.science_denial_war_active());

    ai.enable_science_denial_war();
    ai.disable_science_threat_denial();
    assert!(ai.science_denial_war, "the war flag is left as it was");
    assert!(
        !ai.science_denial_war_active(),
        "the war rung is inert without the base"
    );
}

// ---- the threat model ----------------------------------------------------

#[test]
fn a_pad_or_a_landed_project_makes_a_rival_a_threat_and_the_guards_hold() {
    let mut g = board();
    let ai = denier();
    assert!(
        ai.science_threats(&g, 0).is_empty(),
        "an ordinary rival racing nothing is not a threat"
    );

    // A pad we have seen is the race made physical.
    let (pad_city, pad) = give_pad(&mut g, 1);
    let threats = ai.science_threats(&g, 0);
    assert_eq!(threats.len(), 1);
    assert_eq!(threats[0].rival, 1);
    assert_eq!(threats[0].stages, 0);
    assert_eq!(threats[0].pad, Some((pad_city, pad)));

    // A pad we have never explored is not a target: the raid cannot walk to
    // it and the spy cannot be posted to it.
    g.players[0].explored.remove(&pad);
    assert!(ai.science_threats(&g, 0).is_empty());
    g.players[0].explored.insert(pad);

    // A landed project is a threat with or without a pad in view.
    g.players[2]
        .science_projects
        .insert("launch_earth_satellite".to_string());
    let threats = ai.science_threats(&g, 0);
    assert_eq!(
        threats.iter().map(|t| t.rival).collect::<Vec<_>>(),
        vec![2, 1],
        "a landed stage outranks a bare pad"
    );
    assert_eq!(threats[0].stages, 1);

    // Non-space projects in the same set are not stages.
    g.players[1]
        .science_projects
        .insert("manhattan_project".to_string());
    assert_eq!(ai.science_threats(&g, 0)[1].stages, 0);

    // The stock guards: unmet, dead, a minor, a teammate, our own seat.
    g.players[0].met.remove(&1);
    assert_eq!(ai.science_threats(&g, 0).len(), 1);
    g.record_contact(0, 1);
    g.players[1].alive = false;
    assert_eq!(ai.science_threats(&g, 0).len(), 1);
    g.players[1].alive = true;
    g.players[1].is_minor = true;
    assert_eq!(ai.science_threats(&g, 0).len(), 1);
    g.players[1].is_minor = false;
    g.players[0].team = Some(7);
    g.players[1].team = Some(7);
    assert_eq!(ai.science_threats(&g, 0).len(), 1);
    g.players[0].team = None;
    g.players[1].team = None;
    assert_eq!(ai.science_threats(&g, 0).len(), 2);

    // The layer needs the victory to be winnable and the seat to plan for it.
    g.victory_conditions.science = false;
    assert!(ai.science_threats(&g, 0).is_empty());
    g.victory_conditions.science = true;
    assert!(AdvancedAi::legacy().science_threats(&g, 0).is_empty());
    assert!(
        AdvancedAi::new().science_threats(&g, 0).is_empty(),
        "off, the threat model reads nothing"
    );
}

#[test]
fn a_technology_lead_is_a_threat_only_after_the_pace_turn() {
    let mut g = board();
    let ai = denier();
    let opens = g.standard_duration(SCIENCE_THREAT_TECH_LEAD_TURN);

    give_tech_lead(&mut g, 1, SCIENCE_THREAT_TECH_LEAD);
    g.turn = opens - 1;
    assert!(
        ai.science_threats(&g, 0).is_empty(),
        "an early lead is a scouting result, not a race"
    );
    g.turn = opens;
    let threats = ai.science_threats(&g, 0);
    assert_eq!(threats.len(), 1);
    assert_eq!(threats[0].rival, 1);
    assert_eq!(threats[0].tech_lead, SCIENCE_THREAT_TECH_LEAD);
    assert_eq!(threats[0].pad, None, "a pace threat need not own a pad");

    // One technology short of the bar is nobody's threat, at any turn.
    let caught_up = *g.players[1]
        .techs
        .difference(&g.players[0].techs)
        .next()
        .expect("the rival leads by at least one");
    g.players[0].techs.insert(caught_up);
    assert!(ai.science_threats(&g, 0).is_empty());
}

#[test]
fn threats_rank_by_stages_then_by_the_technology_lead() {
    let mut g = board();
    let ai = denier();
    g.turn = g.standard_duration(SCIENCE_THREAT_TECH_LEAD_TURN);
    give_pad(&mut g, 1);
    give_pad(&mut g, 2);
    give_tech_lead(&mut g, 1, SCIENCE_THREAT_TECH_LEAD);
    assert_eq!(
        ai.science_threats(&g, 0)
            .iter()
            .map(|t| t.rival)
            .collect::<Vec<_>>(),
        vec![1, 2],
        "level on stages, the larger technology lead is more pressing"
    );
    g.players[2]
        .science_projects
        .insert("launch_earth_satellite".to_string());
    assert_eq!(
        ai.science_threats(&g, 0)
            .iter()
            .map(|t| t.rival)
            .collect::<Vec<_>>(),
        vec![2, 1],
        "a landed stage outranks any technology lead"
    );
}

// ---- rung 1: diplomacy ---------------------------------------------------

#[test]
fn passage_and_great_works_are_not_sold_to_a_threat_and_nothing_else_changes() {
    let ai = denier();
    let threats = BTreeSet::from([1]);
    let mut deal = passage_sale(1);
    assert!(!ai.science_denial_deal_allowed(&deal, &threats));
    assert!(
        AdvancedAi::new().science_denial_deal_allowed(&deal, &threats),
        "off, every deal is allowed"
    );
    assert!(
        ai.science_denial_deal_allowed(&deal, &BTreeSet::new()),
        "no threat, no veto"
    );

    deal.direction = "buy".into();
    assert!(
        ai.science_denial_deal_allowed(&deal, &threats),
        "buying their passage does not pay their race"
    );

    deal.direction = "sell".into();
    deal.partner = 2;
    assert!(
        ai.science_denial_deal_allowed(&deal, &threats),
        "a bystander may still buy passage"
    );

    deal.partner = 1;
    deal.item = "silk".into();
    deal.category = "luxury".into();
    assert!(
        ai.science_denial_deal_allowed(&deal, &threats),
        "starving our own treasury does not slow their launch"
    );
    deal.category = "great_work".into();
    assert!(!ai.science_denial_deal_allowed(&deal, &threats));
}

#[test]
fn no_alliance_and_no_research_agreement_with_a_threat() {
    let ai = denier();
    let threats = BTreeSet::from([1]);
    assert!(ai.science_denial_refuses_alliance(&threats, 1));
    assert!(!ai.science_denial_refuses_alliance(&threats, 2));
    assert!(!ai.science_denial_refuses_alliance(&BTreeSet::new(), 1));
    assert!(
        !AdvancedAi::new().science_denial_refuses_alliance(&threats, 1),
        "off, the partner filter is untouched"
    );
}

#[test]
fn the_most_pressing_threat_is_denounced_once_a_turn() {
    let mut g = board();
    let ai = denier();
    give_pad(&mut g, 1);
    give_pad(&mut g, 2);
    g.players[2]
        .science_projects
        .insert("launch_earth_satellite".to_string());

    assert_eq!(
        AdvancedAi::new().science_threat_denunciation(&mut g.clone(), 0),
        None,
        "off, nobody is denounced"
    );
    assert_eq!(
        ai.science_threat_denunciation(&mut g, 0),
        Some(2),
        "the rival with a stage landed is the more pressing"
    );
    assert!(g.players[0].denounced_until.get(&2).copied().unwrap_or(0) > g.turn);
    assert_eq!(
        g.players[0].counters.get("denial_denunciations"),
        Some(&1),
        "the rung records that it reached the board, so a screen row can see it"
    );
    assert_eq!(
        ai.science_threat_denunciation(&mut g, 0),
        Some(1),
        "an active denouncement is not repeated; the next threat is taken"
    );
    assert_eq!(
        ai.science_threat_denunciation(&mut g, 0),
        None,
        "both stand denounced"
    );
}

#[test]
fn an_ally_is_never_denounced_and_the_next_threat_is_taken_instead() {
    let mut g = board();
    let ai = denier();
    give_pad(&mut g, 1);
    give_pad(&mut g, 2);
    // Seat 1 is the more pressing threat and our ally.
    g.players[1]
        .science_projects
        .insert("launch_earth_satellite".to_string());
    let alliance = crate::game::AllianceState {
        kind: "research".to_string(),
        points: 0.0,
        level: 1,
        ends: g.turn + 30,
    };
    g.players[0].alliances.insert(1, alliance.clone());
    g.players[1].alliances.insert(0, alliance);
    assert_eq!(
        ai.science_threat_denunciation(&mut g, 0),
        Some(2),
        "the ally is left out of the ranking; the next threat is denounced"
    );
    assert!(
        g.alliance_with(0, 1).is_some(),
        "an existing alliance is never broken by rung 1"
    );
    assert!(!g.players[0].denounced_until.contains_key(&1));
}

#[test]
fn a_friend_is_never_denounced() {
    let mut g = board();
    let ai = denier();
    give_pad(&mut g, 1);
    g.players[0].friends_until.insert(1, g.turn + 30);
    g.players[1].friends_until.insert(0, g.turn + 30);
    assert_eq!(
        ai.science_threat_denunciation(&mut g, 0),
        None,
        "the engine's own legality refuses it and nothing else is denounced"
    );
}

// ---- rung 2: espionage ---------------------------------------------------

#[test]
fn the_threats_pad_is_the_spy_posting_and_the_disruption_outranks_the_boost() {
    let mut g = board();
    let ai = denier();
    let (pad_city, _) = give_pad(&mut g, 1);
    let bystander = g.player_city_ids(2)[0];

    let pads = ai.science_denial_pad_cities(&g, 0);
    let seats = ai.science_threat_seats(&g, 0);
    assert_eq!(pads, BTreeSet::from([pad_city]));
    assert_eq!(seats, BTreeSet::from([1]));
    assert_eq!(
        AdvancedAi::science_denial_spy_assignment_bonus(&g, 0, 1, &pads, pad_city),
        DENIAL_SPY_ASSIGN_PRIORITY
    );
    assert_eq!(
        AdvancedAi::science_denial_spy_assignment_bonus(&g, 0, 1, &pads, bystander),
        0
    );
    assert_eq!(
        AdvancedAi::new().science_denial_pad_cities(&g, 0),
        BTreeSet::new(),
        "off, the stock assignment table decides"
    );

    assert_eq!(
        AdvancedAi::science_denial_spy_mission_bonus(&seats, 1, "disrupt_rocketry"),
        DENIAL_SPY_MISSION_PRIORITY
    );
    assert_eq!(
        AdvancedAi::science_denial_spy_mission_bonus(&seats, 1, "steal_tech_boost"),
        0.0,
        "only the launch sabotage is promoted"
    );
    assert_eq!(
        AdvancedAi::science_denial_spy_mission_bonus(&seats, 2, "disrupt_rocketry"),
        0.0,
        "a Spaceport that is not racing is an ordinary target"
    );
    assert_eq!(
        AdvancedAi::science_denial_spy_mission_bonus(&BTreeSet::new(), 1, "disrupt_rocketry"),
        0.0,
        "off, the threat set is empty and the bonus is nothing"
    );
}

/// One pad takes one spy. A second spy of ours already in, or ordered to,
/// the threat's launch city withholds the posting bonus from every other
/// spy, so the rest of the network keeps the stock table; the spy that holds
/// the posting still reads its own bonus.
#[test]
fn one_spy_per_pad_and_the_rest_keep_the_stock_table() {
    let mut g = board();
    let ai = denier();
    let (pad_city, _) = give_pad(&mut g, 1);
    let pads = ai.science_denial_pad_cities(&g, 0);
    let spy = |id: u32, city: Option<u32>| crate::game::Spy {
        id,
        owner: 0,
        level: 1,
        promotions: BTreeSet::new(),
        city,
        ready_turn: g.turn + 3,
        mission: None,
        sources_city: None,
        sources_until: 0,
        captured_by: None,
    };
    g.spies.insert(1, spy(1, None));
    g.spies.insert(2, spy(2, None));
    assert_eq!(
        AdvancedAi::science_denial_spy_assignment_bonus(&g, 0, 2, &pads, pad_city),
        DENIAL_SPY_ASSIGN_PRIORITY,
        "nobody is bound to the pad yet"
    );
    // Spy 1 is ordered there: on its way, not yet arrived.
    g.spies.get_mut(&1).unwrap().city = Some(pad_city);
    assert_eq!(
        AdvancedAi::science_denial_spy_assignment_bonus(&g, 0, 2, &pads, pad_city),
        0,
        "a spy already on its way withholds the posting from the rest"
    );
    assert_eq!(
        AdvancedAi::science_denial_spy_assignment_bonus(&g, 0, 1, &pads, pad_city),
        DENIAL_SPY_ASSIGN_PRIORITY,
        "the spy that holds the posting still reads its own bonus"
    );
    // Somebody else's spy in that city binds nothing of ours.
    g.spies.get_mut(&1).unwrap().owner = 2;
    assert_eq!(
        AdvancedAi::science_denial_spy_assignment_bonus(&g, 0, 2, &pads, pad_city),
        DENIAL_SPY_ASSIGN_PRIORITY
    );
}

/// The bonus has to survive the success-chance multiplication the spy pass
/// applies, or promoting it changes nothing. The chances come from the engine
/// (`spy_success_chance`), not from this test; the two stock values are the
/// Science rows of the table in `advanced_spies`.
#[test]
fn the_disruption_bonus_clears_the_success_chance_gap() {
    /// `(GrandStrategy::Science, "disrupt_rocketry")` in `advanced_spies`.
    const STOCK_DISRUPT: f64 = 290.0;
    /// `(GrandStrategy::Science, "steal_tech_boost")` in `advanced_spies`.
    const STOCK_BOOST: f64 = 320.0;

    let mut g = board();
    let (pad_city, pad) = give_pad(&mut g, 1);
    g.spies.insert(
        1,
        crate::game::Spy {
            id: 1,
            owner: 0,
            level: 1,
            promotions: BTreeSet::new(),
            city: Some(pad_city),
            ready_turn: 0,
            mission: None,
            sources_city: None,
            sources_until: 0,
            captured_by: None,
        },
    );
    let chance = |kind: &str| {
        g.spy_success_chance(
            1,
            &crate::game::SpyMission {
                kind: kind.to_string(),
                city: pad_city,
                target: pad,
                started: g.turn,
                ends: g.turn,
            },
        )
    };
    let disrupt = chance("disrupt_rocketry");
    let boost = chance("steal_tech_boost");
    assert!(
        disrupt > 0.0 && disrupt < boost,
        "the launch sabotage is the harder mission ({disrupt} against {boost})"
    );
    assert!(
        STOCK_DISRUPT * disrupt < STOCK_BOOST * boost,
        "the stock table never selects the disruption"
    );
    assert!(
        (STOCK_DISRUPT + DENIAL_SPY_MISSION_PRIORITY) * disrupt > STOCK_BOOST * boost,
        "the denial bonus has to outweigh the lower success chance"
    );
}

// ---- rung 3: the raid ----------------------------------------------------

/// Seat 0 at war with a racing seat 1, with soldiers around its pad.
fn raid_board() -> (Game, Pos) {
    let mut g = board();
    let (_, pad) = give_pad(&mut g, 1);
    for pos in g.map.tiles.keys().copied().collect::<Vec<_>>() {
        g.players[0].explored.insert(pos);
    }
    declare_war(&mut g, 0, 1);
    (g, pad)
}

#[test]
fn the_raid_party_is_two_soldiers_and_never_a_lone_garrison() {
    let (mut g, pad) = raid_board();
    let ai = warrior();
    let ring: Vec<Pos> = g
        .wdisk(pad, 2)
        .into_iter()
        .filter(|pos| {
            *pos != pad
                && g.map
                    .get(*pos)
                    .is_some_and(|tile| !g.rules.is_water(tile) && tile.pos == *pos)
                && g.city_at(*pos).is_none()
        })
        .take(3)
        .collect();
    assert_eq!(ring.len(), 3, "the fixture has three staging tiles");
    let soldiers: Vec<u32> = ring
        .iter()
        .map(|pos| g.spawn_test_unit("warrior", 0, *pos))
        .collect();
    let party = ai.science_denial_raid_party(&g, 0, pad);
    assert_eq!(party.len(), DENIAL_RAID_SIZE);
    assert!(party.iter().all(|uid| soldiers.contains(uid)));

    // The lone garrison of one of our cities is never taken.
    for uid in soldiers {
        g.remove_unit(uid);
    }
    let home = g.cities[&g.player_city_ids(0)[0]].pos;
    g.spawn_test_unit("warrior", 0, home);
    assert!(
        ai.science_denial_raid_party(&g, 0, home).is_empty(),
        "an empty city is the answer to a raid"
    );
}

#[test]
fn the_raid_pillages_the_pad_it_stands_on_and_marches_to_it_otherwise() {
    let (mut g, pad) = raid_board();
    let mut ai = warrior();
    let plan = plan();

    let stander = g.spawn_test_unit("warrior", 0, pad);
    assert_eq!(
        ai.science_denial_raid_step(&mut g, 0, stander, &plan),
        Some(true),
        "a soldier on the pad pillages it"
    );
    assert!(g.map.get(pad).unwrap().pillaged);
    assert_eq!(
        g.players[0].counters.get("denial_pillages"),
        Some(&1),
        "the rung records that it reached a pad, so a screen row can see it"
    );

    // A pillaged pad is finished work: the raid stands down.
    assert_eq!(ai.science_denial_raid_step(&mut g, 0, stander, &plan), None);

    // Repaired, a soldier off the pad marches on it.
    g.map.tiles.get_mut(&pad).unwrap().pillaged = false;
    g.remove_unit(stander);
    let away = g
        .wdisk(pad, 3)
        .into_iter()
        .find(|pos| {
            g.wdist(*pos, pad) == 3
                && g.map.get(*pos).is_some_and(|tile| !g.rules.is_water(tile))
                && g.city_at(*pos).is_none()
        })
        .expect("the fixture has a tile three steps from the pad");
    let marcher = g.spawn_test_unit("warrior", 0, away);
    assert_eq!(
        ai.science_denial_raid_step(&mut g, 0, marcher, &plan),
        Some(true)
    );
    assert!(
        g.wdist(g.units[&marcher].pos, pad) < 3,
        "the step closes on the pad"
    );
}

#[test]
fn the_raid_needs_the_war_and_the_war_tag() {
    let (mut g, pad) = raid_board();
    let plan = plan();
    let uid = g.spawn_test_unit("warrior", 0, pad);

    let mut off = AdvancedAi::targeting(VictoryTarget::Science);
    assert_eq!(
        off.science_denial_raid_step(&mut g, 0, uid, &plan),
        None,
        "off, the ladder is untouched"
    );
    assert!(!g.map.get(pad).unwrap().pillaged);

    let mut base = denier();
    assert_eq!(
        base.science_denial_raid_step(&mut g, 0, uid, &plan),
        None,
        "the base tag alone never raids: rung 3 is `science-denial-war`"
    );
    assert!(!g.map.get(pad).unwrap().pillaged);

    let mut ai = warrior();
    g.at_war.clear();
    assert_eq!(
        ai.science_denial_raid_step(&mut g, 0, uid, &plan),
        None,
        "at peace there is no raid"
    );
    assert!(!g.map.get(pad).unwrap().pillaged);
}

// ---- rung 4: the war -----------------------------------------------------

/// Bank `fraction` of every space project's cost in `cid`, so the projected
/// turns to finish can be steered from the test.
fn bank_space_progress(g: &mut Game, pid: usize, cid: u32, fraction: f64) {
    for project in SPACE_PROJECTS {
        let item = Item::Project {
            project: Name::new(project),
        };
        let cost = g.item_cost_for_city(pid, cid, &item);
        g.cities
            .get_mut(&cid)
            .unwrap()
            .production_progress
            .insert(format!("project:{project}"), cost * fraction);
    }
}

#[test]
fn the_projection_reads_the_launch_city_and_shortens_as_the_race_is_banked() {
    let mut g = board();
    let cid = g.player_city_ids(1)[0];
    let cold = AdvancedAi::science_denial_turns_to_finish(&g, 1, cid)
        .expect("a seat with a launch city can be projected");
    assert_eq!(
        AdvancedAi::science_denial_turns_to_finish(&g, 1, g.player_city_ids(0)[0]),
        None,
        "a city that is not theirs projects nothing"
    );
    bank_space_progress(&mut g, 1, cid, 0.9);
    let warm = AdvancedAi::science_denial_turns_to_finish(&g, 1, cid).unwrap();
    assert!(
        warm < cold && warm >= 0.0,
        "banked production shortens the projection ({warm} vs {cold})"
    );
    bank_space_progress(&mut g, 1, cid, 1.0);
    assert_eq!(
        AdvancedAi::science_denial_turns_to_finish(&g, 1, cid),
        Some(0.0),
        "every project paid for is a finish this turn"
    );
}

#[test]
fn the_denial_war_opens_only_inside_the_horizon_and_behind_our_own_finish() {
    let mut g = board();
    let ai = warrior();
    give_pad(&mut g, 1);
    let theirs = g.player_city_ids(1)[0];
    let ours = g.player_city_ids(0)[0];

    // A rival that has paid for nothing is decades out: no war.
    let threat = ai.science_threats(&g, 0).remove(0);
    assert!(
        !ai.science_denial_war_admissible(&g, 0, &threat),
        "a race that has not started is outside the horizon"
    );

    // Nearly paid up, they are inside it, and we are further out.
    bank_space_progress(&mut g, 1, theirs, 0.99);
    assert!(ai.science_denial_war_admissible(&g, 0, &threat));

    // Our own finish first: a war we would win the game before fighting.
    bank_space_progress(&mut g, 0, ours, 1.0);
    assert!(
        !ai.science_denial_war_admissible(&g, 0, &threat),
        "we finish no later than they do; the production is worth more than the war"
    );

    // Our own finish near: inside the horizon ourselves, even behind them,
    // the last project keeps its production.
    bank_space_progress(&mut g, 1, theirs, 1.0);
    bank_space_progress(&mut g, 0, ours, 0.0);
    let horizon = g.standard_duration(DENIAL_WAR_LAUNCH_HORIZON) as f64;
    let far = AdvancedAi::science_denial_turns_to_finish(&g, 0, ours).unwrap();
    assert!(far > horizon, "the fixture seat is decades out ({far})");
    assert!(ai.science_denial_war_admissible(&g, 0, &threat));
    bank_space_progress(&mut g, 0, ours, 0.995);
    let near = AdvancedAi::science_denial_turns_to_finish(&g, 0, ours).unwrap();
    assert!(
        near <= horizon && near > 0.0,
        "the fixture seat is now inside the horizon ({near})"
    );
    assert!(
        !ai.science_denial_war_admissible(&g, 0, &threat),
        "inside DENIAL_WAR_LAUNCH_HORIZON of our own launch, no war"
    );
    bank_space_progress(&mut g, 1, theirs, 0.99);

    // A threat whose pad we cannot see is not a war target — the raid the
    // war is declared for has nowhere to go.
    bank_space_progress(&mut g, 0, ours, 0.0);
    assert!(ai.science_denial_war_admissible(&g, 0, &threat));
    let padless = ScienceThreat {
        pad: None,
        ..threat
    };
    assert!(!ai.science_denial_war_admissible(&g, 0, &padless));

    // Already at war is rung 3's business, not rung 4's.
    declare_war(&mut g, 0, 1);
    assert!(!ai.science_denial_war_admissible(&g, 0, &threat));
}

#[test]
fn the_declaration_waits_for_the_formal_war_clock_and_then_opens() {
    let mut g = board();
    let mut ai = warrior();
    let (_, pad) = give_pad(&mut g, 1);
    let launch = g.player_city_ids(1)[0];
    bank_space_progress(&mut g, 1, launch, 1.0);
    // The war exists for the raid, so it needs a soldier that can walk to
    // the pad. See `science_denial_raid_can_reach`.
    let staging = g
        .wdisk(pad, 2)
        .into_iter()
        .find(|pos| {
            g.wdist(*pos, pad) == 2
                && g.map.get(*pos).is_some_and(|tile| !g.rules.is_water(tile))
                && g.city_at(*pos).is_none()
        })
        .expect("the fixture has a staging tile beside the pad");
    g.spawn_test_unit("warrior", 0, staging);

    // With no casus belli and no standing denouncement, `preferred_war_opening`
    // answers with the denouncement. That is rung 1's action, not a
    // declaration, so the turn's one war is not spent on it.
    assert!(
        !ai.science_denial_war_diplomacy(&mut g, 0),
        "a denouncement is not a declaration"
    );
    assert!(!g.is_at_war(0, 1));
    assert_eq!(ai.denial_war, None);
    assert!(g.players[0].denounced_until.get(&1).copied().unwrap_or(0) > g.turn);

    // Once the clock has run — and while the denouncement is still active,
    // which is the window `preferred_war_opening` reads — the cheapest legal
    // war opens and the pad is recorded as its objective.
    g.turn += g.standard_duration(20);
    assert!(g.players[0].denounced_until[&1] > g.turn);
    assert!(ai.science_denial_war_diplomacy(&mut g, 0));
    assert!(g.is_at_war(0, 1));
    let war = ai.denial_war.expect("the war is recorded");
    assert_eq!(war.target, 1);
    assert_eq!(war.pad, pad);
    assert_eq!(war.declared, g.turn);
    assert_eq!(g.players[0].counters.get("denial_wars"), Some(&1));

    // A second call while it runs opens nothing more.
    assert!(!ai.science_denial_war_diplomacy(&mut g, 0));
}

/// A board where seat 1 is a threat about to finish and the Formal War clock
/// against it has already run, so `preferred_war_opening` answers with a
/// declaration rather than a denouncement. Returns the pad.
fn war_ready_board(ai: &mut AdvancedAi) -> (Game, Pos) {
    let mut g = board();
    let (_, pad) = give_pad(&mut g, 1);
    let launch = g.player_city_ids(1)[0];
    bank_space_progress(&mut g, 1, launch, 1.0);
    // The first pass denounces; the clock runs; the declaration is then legal.
    assert!(!ai.science_denial_war_diplomacy(&mut g, 0));
    g.turn += g.standard_duration(20);
    assert!(g.players[0].denounced_until[&1] > g.turn);
    (g, pad)
}

/// The base tag alone never declares: a threat about to finish, the clock
/// run, an army beside the pad — and `science-threat-denial` on its own
/// spends nothing but the denunciation.
#[test]
fn the_base_tag_alone_never_declares_a_war() {
    let mut ai = warrior();
    let (mut g, pad) = war_ready_board(&mut ai);
    let staging = g
        .wdisk(pad, 2)
        .into_iter()
        .find(|pos| {
            g.wdist(*pos, pad) == 2
                && g.map.get(*pos).is_some_and(|tile| !g.rules.is_water(tile))
                && g.city_at(*pos).is_none()
        })
        .expect("the fixture has a staging tile beside the pad");
    g.spawn_test_unit("warrior", 0, staging);
    g.spawn_test_unit("warrior", 0, staging);
    let mut base = denier();
    assert!(!base.science_denial_war_diplomacy(&mut g, 0));
    assert!(!g.is_at_war(0, 1));
    assert_eq!(base.denial_war, None);
    assert_eq!(g.players[0].counters.get("denial_wars"), None);
    // The same board and the war tag: the declaration opens.
    assert!(ai.science_denial_war_diplomacy(&mut g, 0));
    assert!(g.is_at_war(0, 1));
}

#[test]
fn no_war_is_declared_for_a_pad_no_soldier_can_walk_to() {
    let mut ai = warrior();
    let (mut g, pad) = war_ready_board(&mut ai);

    // No army at all: the declaration would buy grievances and no denial.
    assert!(!ai.science_denial_war_diplomacy(&mut g, 0));
    assert!(!g.is_at_war(0, 1));
    assert_eq!(ai.denial_war, None);

    // The lone garrison of our own city does not count as the march either.
    let home = g.cities[&g.player_city_ids(0)[0]].pos;
    let garrison = g.spawn_test_unit("warrior", 0, home);
    assert!(!ai.science_denial_war_diplomacy(&mut g, 0));
    assert!(!g.is_at_war(0, 1));

    // A second soldier frees the first, and the war opens.
    g.spawn_test_unit("warrior", 0, home);
    let _ = garrison;
    assert!(ai.science_denial_war_diplomacy(&mut g, 0));
    assert!(g.is_at_war(0, 1));
    assert_eq!(ai.denial_war.map(|war| war.pad), Some(pad));
}

#[test]
fn the_denial_war_is_refused_by_the_existing_war_gates() {
    let mut ai = warrior();
    let (mut g, _) = war_ready_board(&mut ai);
    let home = g.cities[&g.player_city_ids(0)[0]].pos;
    g.spawn_test_unit("warrior", 0, home);
    g.spawn_test_unit("warrior", 0, home);
    ai.enable_war_needs_a_treasury();
    // A treasury already running dry refuses every war, this one included.
    g.players[0].gold = 0.0;
    g.players[0].gold_per_turn = -50.0;
    assert!(
        !ai.science_denial_war_diplomacy(&mut g, 0),
        "war-needs-a-treasury holds the declaration"
    );
    assert!(!g.is_at_war(0, 1));

    // And one-war-at-a-time holds it while another major war burns.
    g.players[0].gold = 5_000.0;
    g.players[0].gold_per_turn = 100.0;
    ai.enable_one_war_at_a_time();
    declare_war(&mut g, 0, 2);
    ai.one_war_observe(&g, 0);
    assert!(!ai.science_denial_war_diplomacy(&mut g, 0));
    assert!(!g.is_at_war(0, 1));
}

#[test]
fn the_denial_war_closes_when_the_pad_is_pillaged() {
    let mut g = board();
    let mut ai = warrior();
    let (_, pad) = give_pad(&mut g, 1);
    declare_war(&mut g, 0, 1);
    ai.denial_war = Some(DenialWar {
        target: 1,
        declared: g.turn,
        pad,
    });

    // Before the engine's minimum war length nothing is proposed.
    assert!(!ai.science_denial_war_diplomacy(&mut g, 0));
    assert!(ai.peace_offers.is_empty());

    g.turn += g.standard_duration(RAID_PEACE_EARLIEST);
    assert!(
        !ai.science_denial_war_diplomacy(&mut g, 0),
        "a standing pad is unfinished work"
    );
    assert!(ai.peace_offers.is_empty());

    g.map.tiles.get_mut(&pad).unwrap().pillaged = true;
    assert!(!ai.science_denial_war_diplomacy(&mut g, 0));
    assert!(
        ai.peace_offers.contains(&1),
        "the pad is pillaged; the war is over"
    );
}

#[test]
fn the_denial_war_closes_when_it_has_run_its_course() {
    let mut g = board();
    let mut ai = warrior();
    let (_, pad) = give_pad(&mut g, 1);
    declare_war(&mut g, 0, 1);
    ai.denial_war = Some(DenialWar {
        target: 1,
        declared: g.turn,
        pad,
    });
    g.turn += g.standard_duration(DENIAL_WAR_MAX_TURNS);
    assert!(!ai.science_denial_war_diplomacy(&mut g, 0));
    assert!(ai.peace_offers.contains(&1));
}

#[test]
fn a_concluded_denial_war_is_forgotten_and_the_gene_off_clears_it() {
    let mut g = board();
    let mut ai = warrior();
    let (_, pad) = give_pad(&mut g, 1);
    ai.denial_war = Some(DenialWar {
        target: 1,
        declared: g.turn,
        pad,
    });
    // Never declared, or peace already made: not at war, so it is dropped.
    assert!(!ai.science_denial_war_diplomacy(&mut g, 0));
    assert_eq!(ai.denial_war, None);

    ai.denial_war = Some(DenialWar {
        target: 1,
        declared: g.turn,
        pad,
    });
    ai.disable_science_denial_war();
    assert!(!ai.science_denial_war_diplomacy(&mut g, 0));
    assert_eq!(
        ai.denial_war, None,
        "the war tag off, the layer holds no state"
    );

    ai.enable_science_denial_war();
    ai.denial_war = Some(DenialWar {
        target: 1,
        declared: g.turn,
        pad,
    });
    ai.disable_science_threat_denial();
    assert!(!ai.science_denial_war_diplomacy(&mut g, 0));
    assert_eq!(
        ai.denial_war, None,
        "the base off, the war rung is inert too"
    );
}

// ---- the off path --------------------------------------------------------

#[test]
fn off_no_entry_point_reads_the_board() {
    let mut g = board();
    give_pad(&mut g, 1);
    g.players[1]
        .science_projects
        .insert("launch_mars_colony".to_string());
    declare_war(&mut g, 0, 1);
    let pad = g.cities[&g.player_city_ids(1)[0]]
        .districts
        .get(crate::name!("spaceport"))
        .copied()
        .unwrap();
    let uid = g.spawn_test_unit("warrior", 0, pad);
    let plan = plan();

    let mut off = AdvancedAi::targeting(VictoryTarget::Science);
    assert!(off.science_threats(&g, 0).is_empty());
    assert!(off.science_threat_seats(&g, 0).is_empty());
    assert!(off.science_denial_deal_allowed(&passage_sale(1), &BTreeSet::from([1])));
    assert!(!off.science_denial_refuses_alliance(&BTreeSet::from([1]), 1));
    assert!(off.science_denial_pad_cities(&g, 0).is_empty());
    assert_eq!(off.science_denial_raid_step(&mut g, 0, uid, &plan), None);
    assert!(!off.science_denial_war_diplomacy(&mut g, 0));
    assert_eq!(off.science_threat_denunciation(&mut g, 0), None);
    assert!(!g.map.get(pad).unwrap().pillaged);
    assert!(off.peace_offers.is_empty());
    assert_eq!(GrandStrategy::Science, GrandStrategy::Science);
}
