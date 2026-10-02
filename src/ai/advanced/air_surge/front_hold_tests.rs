//! The surge keeps its own war: the declaring frame, and the stalled-war
//! peace offer. Live King 20260930T211803Z lost the appointment both ways.
use super::*;

fn fixture() -> (Game, AdvancedAi, u32) {
    fixture_with(2)
}

fn fixture_with(players: usize) -> (Game, AdvancedAi, u32) {
    let mut g = Game::new_full(players, 40, 24, 936077, 650, 0, false);
    for uid in g.units.keys().copied().collect::<Vec<_>>() {
        g.remove_unit(uid);
    }
    g.barb_camps.clear();
    g.barb_naval_camps.clear();
    for tile in g.map.tiles.values_mut() {
        tile.terrain = crate::name!("grassland");
        tile.feature = None;
        tile.resource = None;
        tile.hills = false;
    }
    g.found_city_for(0, (12, 12), None);
    g.found_city_for(0, (12, 18), None);
    let target = g.found_city_for(1, (20, 12), None);
    g.found_city_for(1, (26, 16), None);
    g.record_contact(0, 1);
    g.players[0].techs.extend(
        g.rules.tech_ancestors[AIR_SURGE_GOAL_TECH]
            .iter()
            .map(|tech| Name::new(tech)),
    );
    g.players[0].techs.insert(Name::new(AIR_SURGE_GOAL_TECH));
    g.players[0]
        .strategic_resources
        .insert(crate::name!("aluminum"), 400.0);
    g.at_war.clear();
    g.current = 0;
    g.turn = 170;
    for pos in [(12, 12), (11, 12)] {
        g.spawn_test_unit("bomber", 0, pos);
    }
    for pos in [(13, 12), (13, 13)] {
        g.spawn_test_unit("cuirassier", 0, pos);
    }
    let mut ai = AdvancedAi::targeting(VictoryTarget::Domination);
    ai.enable_air_surge_2();
    ai.air_surge_plan = Some(AirSurge {
        target_player: 1,
        objective_city: target,
        objective_pos: g.cities[&target].pos,
        body_unit: crate::name!("cuirassier"),
        body_is_cavalry: true,
        opened_at_war: false,
        phase: AirSurgePhase::Exploit,
        appointed_turn: 150,
        tech_turn: Some(160),
        declared_turn: Some(170),
        last_reviewed_turn: 170,
        recovery_assessments: 0,
    });
    (g, ai, target)
}

/// Turn 183 of the live game: frame one declared, frame two's fresh board had
/// not exported the war yet, and the wing stood down with its cooldown.
#[test]
fn the_declaring_frame_does_not_read_as_peace() {
    for (declared, survives) in [(170, true), (169, true), (168, false)] {
        let (g, mut ai, _) = fixture();
        ai.air_surge_plan.as_mut().unwrap().declared_turn = Some(declared);
        assert!(!g.is_at_war(0, 1));
        ai.maintain_air_surge(&g, 0);
        assert_eq!(ai.air_surge_plan.is_some(), survives, "declared {declared}");
        if survives {
            assert_eq!(
                ai.air_surge_plan.as_ref().unwrap().phase,
                AirSurgePhase::Exploit,
                "an in-flight declaration must not re-open Strike and declare again"
            );
            assert!(ai.air_surge_census.aborts.is_empty());
        } else {
            assert_eq!(ai.air_surge_census.aborts.get("peace closed the war"), Some(&1));
        }
    }
}

#[test]
fn the_declaring_frame_grace_is_version_two_only() {
    let (g, mut ai, _) = fixture();
    ai.disable_air_surge_2();
    ai.air_surge = true;
    ai.maintain_air_surge(&g, 0);
    assert!(ai.air_surge_plan.is_none());
}

fn at_war(g: &mut Game) {
    g.at_war.insert((0, 1));
    g.at_war.insert((1, 0));
}

#[test]
fn the_surge_front_is_held_once_the_wing_is_near() {
    let (mut g, mut ai, _) = fixture();
    at_war(&mut g);
    ai.air_surge_status = ai.air_surge_status(&g, 0, ai.air_surge_plan.as_ref().unwrap());
    assert!(ai.air_surge_status.bombers >= 2);
    assert!(ai.air_surge_holds_front(&g, 0, 1));

    // Two techs out still holds; three does not.
    for uid in g.player_unit_ids(0) {
        if AdvancedAi::air_surge_is_bomber(&g, g.units[&uid].kind) {
            g.remove_unit(uid);
        }
    }
    ai.air_surge_status = AirSurgeStatus::default();
    for tech in ["advanced_flight", "radio"] {
        g.players[0].techs.remove(&Name::new(tech));
    }
    assert_eq!(AdvancedAi::air_surge_missing_techs(&g, 0), 2);
    assert!(ai.air_surge_holds_front(&g, 0, 1));
    g.players[0].techs.remove(&crate::name!("flight"));
    assert!(!ai.air_surge_holds_front(&g, 0, 1));
}

#[test]
fn the_front_hold_needs_the_domination_surge_on_that_rival() {
    for case in ["peace", "legacy", "lane", "no_plan", "other_target"] {
        let (mut g, mut ai, _) = fixture();
        at_war(&mut g);
        ai.air_surge_status = ai.air_surge_status(&g, 0, ai.air_surge_plan.as_ref().unwrap());
        match case {
            "peace" => g.at_war.clear(),
            "legacy" => {
                ai.disable_air_surge_2();
                ai.air_surge = true;
            }
            "lane" => ai.victory_target = Some(VictoryTarget::Science),
            "no_plan" => ai.air_surge_plan = None,
            "other_target" => ai.air_surge_plan.as_mut().unwrap().target_player = 0,
            _ => unreachable!(),
        }
        assert!(!ai.air_surge_holds_front(&g, 0, 1), "{case}");
    }
}

/// Turns 132 and 168 of the live game: "the war has stalled" at 2.7x and
/// 1.7x the target's power, against the civilization the wing was built for.
#[test]
fn a_stalled_war_against_the_armed_surge_target_is_not_offered_peace() {
    let offers = |hold: bool| {
        let (mut g, mut ai, target) = fixture();
        at_war(&mut g);
        // Short of crushed (`domination_front_crushed`): the live fronts
        // were 2.7x and 1.7x.
        for pos in [(36, 20), (36, 21)] {
            g.spawn_test_unit("cuirassier", 1, pos);
        }
        g.players[0].gold = 0.0;
        g.players[0].gold_per_turn = -11.0;
        ai.enable_peace_when_war_does_not_pay();
        ai.major_war_since = Some(g.turn - 40);
        ai.last_campaign_progress = g.turn - 40;
        if !hold {
            ai.air_surge_plan = None;
        }
        ai.air_surge_status = ai
            .air_surge_plan
            .as_ref()
            .map(|plan| ai.air_surge_status(&g, 0, plan))
            .unwrap_or_default();
        let plan = StrategicPlan {
            strategy: GrandStrategy::Conquest,
            target_player: Some(1),
            target_city: Some(target),
            threatened_city: None,
            desired_cities: 4,
            assessed_turn: g.turn,
            rush: false,
        };
        ai.advanced_diplomacy(&mut g, 0, &plan);
        ai.peace_offers.contains(&1)
    };
    assert!(offers(false), "the control: this war is stalled and unpaid");
    assert!(!offers(true), "the surge's own front keeps its war");
}

/// The same front refuses the target's own peace offer: an accepted treaty
/// stands the wing down and blocks a new appointment for thirty turns.
#[test]
fn the_armed_surge_target_s_peace_offer_is_refused() {
    let accepts = |hold: bool| {
        let (mut g, mut ai, target) = fixture();
        at_war(&mut g);
        if !hold {
            ai.air_surge_plan = None;
        }
        ai.air_surge_status = ai
            .air_surge_plan
            .as_ref()
            .map(|plan| ai.air_surge_status(&g, 0, plan))
            .unwrap_or_default();
        g.pending_deals.push(crate::game::DiplomaticDeal {
            id: 7001,
            from: 1,
            to: 0,
            give_gold: 0.0,
            request_gold: 0.0,
            open_borders: false,
            friendship: false,
            peace: true,
            alliance: None,
            defensive_pact: false,
            joint_war_target: None,
            promise: None,
            demand: false,
            expires: g.turn + 5,
        });
        // A turn whose plan is elsewhere: the ordinary valuation takes a
        // white peace from a rival it is not campaigning against.
        let plan = StrategicPlan {
            strategy: GrandStrategy::Expansion,
            target_player: None,
            target_city: None,
            threatened_city: None,
            desired_cities: 4,
            assessed_turn: g.turn,
            rush: false,
        };
        let _ = target;
        ai.advanced_diplomacy(&mut g, 0, &plan);
        !g.is_at_war(0, 1)
    };
    assert!(!accepts(true), "the surge's own front keeps its war");
    assert!(accepts(false), "the control accepts a white peace");
}

/// Live King 20260930T211803Z aimed the wing at four walled border towns
/// and never at a capital. A Domination wing prices walls as the thing it is
/// built to remove and a founding capital as the victory itself.
#[test]
fn a_domination_wing_prefers_a_walled_capital_to_a_border_town() {
    let (mut g, ai, capital) = fixture();
    assert!(g.cities[&capital].is_capital);
    let town = g
        .cities
        .values()
        .find(|city| city.owner == 1 && !city.is_capital)
        .unwrap()
        .id;
    for cid in [capital, town] {
        let city = g.cities.get_mut(&cid).unwrap();
        city.wall_hp = if cid == capital { 400 } else { 100 };
    }
    let (capital_city, town_city) = (g.cities[&capital].clone(), g.cities[&town].clone());
    let air = |ai: &AdvancedAi, city| ai.air_surge_objective_value(&g, 0, city);
    assert!(air(&ai, &capital_city) < air(&ai, &town_city));

    // Outside the Domination lane it is the ground ranking, unchanged.
    let mut other = ai.clone();
    other.victory_target = Some(VictoryTarget::Culture);
    assert_eq!(
        air(&other, &capital_city),
        other.campaign_city_value(&g, 0, &capital_city, GrandStrategy::Conquest)
    );
}

/// Live King 20260930T225143Z: losing the one base in range of Groningen
/// stood the surge down with its cooldown while three other Dutch cities
/// were still under the wing. Before the declaration it re-aims instead.
#[test]
fn an_objective_out_of_reach_re_aims_the_undeclared_surge() {
    let (g, mut ai, capital) = fixture();
    let far = g
        .cities
        .values()
        .find(|city| city.owner == 1 && !city.is_capital)
        .unwrap()
        .clone();
    assert!(!AdvancedAi::air_surge_in_range(&g, 0, far.pos));
    {
        let plan = ai.air_surge_plan.as_mut().unwrap();
        plan.objective_city = far.id;
        plan.objective_pos = far.pos;
        plan.declared_turn = None;
        plan.phase = AirSurgePhase::Arm;
    }
    ai.maintain_air_surge(&g, 0);
    let plan = ai.air_surge_plan.as_ref().expect("the surge re-aims");
    assert_eq!(plan.objective_city, capital);
    assert_eq!(plan.appointed_turn, 150, "the clocks carry over");
    assert_eq!(plan.tech_turn, Some(160));
    assert!(ai.air_surge_census.aborts.is_empty());

    // Version one keeps its stand-down.
    let (g, mut legacy, _) = fixture();
    legacy.disable_air_surge_2();
    legacy.air_surge = true;
    {
        let plan = legacy.air_surge_plan.as_mut().unwrap();
        plan.objective_city = far.id;
        plan.objective_pos = far.pos;
        plan.declared_turn = None;
        plan.phase = AirSurgePhase::Arm;
    }
    legacy.maintain_air_surge(&g, 0);
    assert!(legacy.air_surge_plan.is_none());
}

/// A counter-surge whose war ends in peace re-arms against a legal target
/// instead of standing down: live 20260930T221624Z lost its surge to
/// Sweden's peace five turns after appointing it.
#[test]
fn a_counter_surge_closed_by_peace_re_arms_as_an_elective_surge() {
    let (g, mut ai, _) = fixture();
    {
        let plan = ai.air_surge_plan.as_mut().unwrap();
        plan.opened_at_war = true;
        plan.declared_turn = None;
    }
    assert!(!g.is_at_war(0, 1));
    ai.maintain_air_surge(&g, 0);
    let plan = ai.air_surge_plan.as_ref().expect("the surge re-arms");
    assert!(!plan.opened_at_war && plan.declared_turn.is_none());
    assert_eq!(plan.appointed_turn, 150);
    assert!(ai.air_surge_census.aborts.is_empty());
}

/// Live King 20261001T010043Z: every Aluminum deposit lay in a rival's
/// borders and the wing stood down twice without a Bomber. A starved
/// Domination wing aims at the city that holds the metal and may open that
/// war with its ground capture package alone.
#[test]
fn a_wing_starved_of_aluminum_goes_to_take_the_city_that_holds_it() {
    let (mut g, mut ai, capital) = fixture();
    // No metal of our own and no Bombers; the metal is revealed.
    g.players[0].strategic_resources.clear();
    for uid in g.player_unit_ids(0) {
        if AdvancedAi::air_surge_is_bomber(&g, g.units[&uid].kind) {
            g.remove_unit(uid);
        }
    }
    g.players[0].explored.extend(g.map.tiles.keys().copied());
    let town = g
        .cities
        .values()
        .find(|city| city.owner == 1 && !city.is_capital)
        .unwrap()
        .clone();
    // The rival town holds a deposit next to its centre.
    let deposit = *town
        .owned_tiles
        .iter()
        .find(|pos| **pos != town.pos)
        .unwrap();
    g.map.tiles.get_mut(&deposit).unwrap().resource = Some(crate::name!("aluminum"));
    assert!(ai.air_surge_metal_starved(&g, 0));
    // The town is out of a Bomber's reach from home in this fixture; bring
    // a base within ten tiles of it.
    g.found_city_for(0, (town.pos.0 - 6, town.pos.1), None);
    {
        let plan = ai.air_surge_plan.as_mut().unwrap();
        plan.objective_city = capital;
        plan.objective_pos = g.cities[&capital].pos;
        plan.declared_turn = None;
        plan.phase = AirSurgePhase::Arm;
    }
    ai.maintain_air_surge(&g, 0);
    let plan = ai.air_surge_plan.as_ref().expect("the surge holds");
    assert_eq!(plan.objective_pos, town.pos, "it re-aims at the metal");
    assert!(ai.air_surge_census.aborts.is_empty());

    // With four capture bodies the grab opens without a single Bomber.
    for pos in [(13, 13), (12, 13)] {
        g.spawn_test_unit("cuirassier", 0, pos);
    }
    ai.maintain_air_surge(&g, 0);
    assert!(ai.air_surge_status.metal_grab_ready);
    assert_eq!(ai.air_surge_plan.as_ref().unwrap().phase, AirSurgePhase::Strike);

    // A deposit in our own land is mined, not conquered.
    let own = g.cities.values().find(|city| city.owner == 0).unwrap().clone();
    let home = *own.owned_tiles.iter().find(|pos| **pos != own.pos).unwrap();
    g.map.tiles.get_mut(&home).unwrap().resource = Some(crate::name!("aluminum"));
    assert!(!ai.air_surge_metal_starved(&g, 0));
}

/// Live King 20261001T010043Z: the surge aimed at Warsaw while the land
/// campaign marched on Kraków. Until the wing flies it joins the campaign's
/// city when that city is under its reach; a ready wing keeps its own.
#[test]
fn an_unready_wing_joins_the_land_campaigns_city() {
    let (mut g, mut ai, capital) = fixture();
    let town = g
        .cities
        .values()
        .find(|city| city.owner == 1 && !city.is_capital)
        .unwrap()
        .clone();
    g.found_city_for(0, (town.pos.0 - 6, town.pos.1), None);
    {
        let plan = ai.air_surge_plan.as_mut().unwrap();
        plan.objective_city = capital;
        plan.objective_pos = g.cities[&capital].pos;
        plan.declared_turn = None;
        plan.phase = AirSurgePhase::Arm;
    }
    let campaign = StrategicPlan {
        strategy: GrandStrategy::Conquest,
        target_player: Some(1),
        target_city: Some(town.id),
        threatened_city: None,
        desired_cities: 4,
        assessed_turn: g.turn,
        rush: false,
    };
    // A ready wing keeps its own objective.
    ai.air_surge_status = ai.air_surge_status(&g, 0, ai.air_surge_plan.as_ref().unwrap());
    assert!(ai.air_surge_status.wing_ready());
    ai.air_surge_adopt_campaign(&g, 0, &campaign);
    assert_eq!(ai.air_surge_plan.as_ref().unwrap().objective_city, capital);

    // While it is still being built, it joins the campaign.
    ai.air_surge_status = AirSurgeStatus::default();
    ai.air_surge_adopt_campaign(&g, 0, &campaign);
    let plan = ai.air_surge_plan.as_ref().unwrap();
    assert_eq!(plan.objective_pos, town.pos);
    assert_eq!(plan.appointed_turn, 150, "the clocks carry over");
}

/// Live King 20261001T022028Z and 024402Z lost Religious victories at turns
/// 147 and 168 with the wing five turns out and aimed elsewhere. When the
/// denial layer names a rival, the surge turns on it before declaring, and a
/// surge aimed elsewhere never pulls the land campaign off the denial war.
#[test]
fn the_wing_turns_on_the_rival_whose_victory_must_be_denied() {
    let mut g = Game::new_full(3, 40, 24, 37_201, 400, 0, false);
    for uid in g.units.keys().copied().collect::<Vec<_>>() {
        g.remove_unit(uid);
    }
    g.barb_camps.clear();
    g.barb_naval_camps.clear();
    for tile in g.map.tiles.values_mut() {
        tile.terrain = crate::name!("grassland");
        tile.feature = None;
        tile.resource = None;
        tile.hills = false;
    }
    g.found_city_for(0, (12, 12), None);
    g.found_city_for(0, (12, 18), None);
    let leader = g.found_city_for(1, (20, 12), None);
    let other = g.found_city_for(2, (20, 18), None);
    for rival in 1..3 {
        g.record_contact(0, rival);
    }
    g.players[0].techs.extend(
        g.rules.tech_ancestors[AIR_SURGE_GOAL_TECH]
            .iter()
            .map(|tech| Name::new(tech)),
    );
    g.players[0]
        .strategic_resources
        .insert(crate::name!("aluminum"), 400.0);
    for pos in [(13, 12), (13, 13)] {
        g.spawn_test_unit("cuirassier", 0, pos);
    }
    g.turn = 170;
    g.current = 0;
    // Player 1 is about to win on Culture: its tourists against our
    // domestic ones, as the urgent-denial opening tests stage it.
    let observed = std::sync::Arc::make_mut(&mut g.observed_public_empire_stats);
    observed.entry(0).or_default().domestic_tourists = Some(100);
    observed.entry(1).or_default().foreign_tourists = Some(85);
    let mut ai = AdvancedAi::targeting(VictoryTarget::Domination);
    ai.enable_deny_while_targeted();
    ai.enable_denial_outranks_expansion();
    ai.enable_counter_in_lane();
    ai.enable_air_surge_2();
    assert_eq!(ai.air_surge_denial_rival(&g, 0), Some(1), "{:?}", ai.denial_target(&g, 0));
    ai.air_surge_plan = Some(AirSurge {
        target_player: 2,
        objective_city: other,
        objective_pos: (20, 18),
        body_unit: crate::name!("cuirassier"),
        body_is_cavalry: true,
        opened_at_war: false,
        phase: AirSurgePhase::Arm,
        appointed_turn: 150,
        tech_turn: None,
        declared_turn: None,
        last_reviewed_turn: 170,
        recovery_assessments: 0,
    });
    // Aimed elsewhere, the surge does not take the land campaign.
    ai.air_surge_status = ai.air_surge_status(&g, 0, ai.air_surge_plan.as_ref().unwrap());
    assert!(ai.air_surge_status.denial_elsewhere);
    let mut campaign = StrategicPlan {
        strategy: GrandStrategy::Conquest,
        target_player: Some(1),
        target_city: Some(leader),
        threatened_city: None,
        desired_cities: 4,
        assessed_turn: g.turn,
        rush: false,
    };
    ai.air_surge_plan.as_mut().unwrap().phase = AirSurgePhase::Strike;
    ai.apply_air_surge_to_strategy(&mut campaign);
    assert_eq!(campaign.target_player, Some(1));
    assert_eq!(campaign.target_city, Some(leader));

    // Before its declaration it re-aims at the denial rival.
    ai.air_surge_plan.as_mut().unwrap().phase = AirSurgePhase::Arm;
    ai.maintain_air_surge(&g, 0);
    let plan = ai
        .air_surge_plan
        .as_ref()
        .unwrap_or_else(|| panic!("the surge holds: {:?}", ai.air_surge_census.aborts));
    assert_eq!(plan.target_player, 1);
    assert_eq!(plan.objective_pos, (20, 12));
    assert_eq!(plan.appointed_turn, 150);
}

/// Live King 20261001T030914Z mined two Aluminum a turn, banked 25 and flew
/// a two-Bomber wing one sortie at a time. A bank pays for each Bomber past
/// the income: its training and its upkeep through the grace window.
#[test]
fn a_banked_stockpile_buys_bombers_past_the_income() {
    let (mut g, _, _) = fixture();
    std::sync::Arc::make_mut(&mut g.observed_strategic_income_adjustments)
        .entry(0)
        .or_default()
        .insert(crate::name!("aluminum"), 2.0);
    let bomber = AdvancedAi::air_surge_bomber(&g, 0).unwrap();
    let spec = g.rules.units[bomber].clone();
    let grace = g.standard_duration(AIR_SURGE_ALUMINUM_GRACE) as f64;
    let per_extra = spec.resource_cost + spec.resource_maintenance * grace;
    let goal = |g: &mut Game, banked: f64| {
        g.players[0]
            .strategic_resources
            .insert(crate::name!("aluminum"), banked);
        AdvancedAi::air_surge_bomber_goal(g, 0)
    };
    // The fixture's two standing Bombers already draw two a turn; count only
    // what is left for the wing goal.
    let income = g.strategic_resource_rate(0, "aluminum");
    let sustainable = ((income / spec.resource_maintenance).floor() as usize).min(AIR_SURGE_BOMBERS);
    assert!(sustainable >= AIR_SURGE_LAUNCH_BOMBERS, "precondition: {income}");
    assert_eq!(goal(&mut g, 0.0), sustainable);
    assert_eq!(
        goal(&mut g, per_extra * 2.0),
        (sustainable + 2).min(AIR_SURGE_BOMBERS)
    );
    assert_eq!(goal(&mut g, 10_000.0), AIR_SURGE_BOMBERS, "the ceiling holds");
}

/// Live King 20261001T080758Z: Bombers against Pella from turn 156 while the
/// land campaign was aimed at the Zulu. `one_war_at_a_time` offered Macedon
/// peace every turn from 157 as its power fell from 167 to 18, Macedon took
/// it at 167, and the surge stood down. The wing's own front is not a second
/// front.
#[test]
fn one_war_does_not_offer_the_wing_s_front_peace() {
    let offers = |hold: bool| {
        let (mut g, mut ai, target) = fixture_with(3);
        g.found_city_for(2, (32, 6), None);
        g.record_contact(0, 2);
        at_war(&mut g);
        g.at_war.insert((0, 2));
        g.at_war.insert((2, 0));
        ai.one_war_at_a_time = true;
        ai.one_war = Some(crate::ai::advanced::one_war::OneWarFront {
            target: 2,
            since: g.turn - 10,
            ledger: (0, 0, 0, 0),
            window: Default::default(),
            tide_against_since: None,
            city_health: Default::default(),
            sieges_advancing: 0,
        });
        if !hold {
            ai.air_surge_plan = None;
        }
        ai.air_surge_status = ai
            .air_surge_plan
            .as_ref()
            .map(|plan| ai.air_surge_status(&g, 0, plan))
            .unwrap_or_default();
        assert_eq!(
            ai.one_war_peace(&g, 0, 1),
            Some(crate::ai::advanced::one_war::OneWarPeace::SecondFront)
        );
        let plan = StrategicPlan {
            strategy: GrandStrategy::Conquest,
            target_player: Some(2),
            target_city: g.player_city_ids(2).first().copied(),
            threatened_city: None,
            desired_cities: 4,
            assessed_turn: g.turn,
            rush: false,
        };
        let _ = target;
        ai.advanced_diplomacy(&mut g, 0, &plan);
        ai.peace_offers.contains(&1)
    };
    assert!(offers(false), "the control: a second front is offered peace");
    assert!(!offers(true), "the wing's front keeps its war");
}

/// The same game, turn 157: Macedon opened the war on a frame that read
/// Recovery for a threatened Cuenca, and the Recovery clause offered the
/// wing's target peace at 842 power against 167. Only the outmatched clause
/// may offer the wing's front peace.
#[test]
fn a_recovery_reading_does_not_offer_the_wing_s_front_peace() {
    let offers = |hold: bool| {
        let (mut g, mut ai, _) = fixture();
        at_war(&mut g);
        if !hold {
            ai.air_surge_plan = None;
        }
        ai.air_surge_status = ai
            .air_surge_plan
            .as_ref()
            .map(|plan| ai.air_surge_status(&g, 0, plan))
            .unwrap_or_default();
        let home = g.player_city_ids(0)[0];
        let plan = StrategicPlan {
            strategy: GrandStrategy::Recovery,
            target_player: None,
            target_city: None,
            threatened_city: Some(home),
            desired_cities: 4,
            assessed_turn: g.turn,
            rush: false,
        };
        ai.advanced_diplomacy(&mut g, 0, &plan);
        ai.peace_offers.contains(&1)
    };
    assert!(offers(false), "the control: Recovery offers a non-target peace");
    assert!(!offers(true), "the wing's front keeps its war");
}
