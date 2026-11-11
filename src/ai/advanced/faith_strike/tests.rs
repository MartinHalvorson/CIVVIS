use super::*;

/// Our spearmen near our city at (2, 8), and rival 1 with a founded faith at
/// peace with us. The tiles are open grassland.
fn fixture() -> (Game, AdvancedAi) {
    let mut g = Game::new_full(4, 40, 24, 372101, 250, 0, false);
    for uid in g.units.keys().copied().collect::<Vec<_>>() {
        g.remove_unit(uid);
    }
    g.barb_camps.clear();
    for tile in g.map.tiles.values_mut() {
        tile.terrain = crate::name!("grassland");
        tile.feature = None;
        tile.resource = None;
        tile.hills = false;
    }
    g.found_city_for(0, (2, 8), None);
    g.found_city_for(1, (30, 8), None);
    g.found_city_for(2, (18, 18), None);
    g.players[1].religion = Some("Islam".into());
    for rival in 1..4 {
        g.record_contact(0, rival);
    }
    g.at_war.clear();
    g.current = 0;
    g.turn = 83;
    g.players[0].gold = 10000.0;
    g.players[0].gold_per_turn = 10.0;
    let mut ai = AdvancedAi::targeting(VictoryTarget::Religion);
    ai.faith_strike_wipes_the_spreaders = true;
    (g, ai)
}

fn soldier(g: &mut Game, at: Pos) -> u32 {
    g.spawn_test_unit("spearman", 0, at)
}

fn spreader(g: &mut Game, kind: &str, at: Pos) -> u32 {
    let uid = g.spawn_test_unit(kind, 1, at);
    g.units.get_mut(&uid).unwrap().religion = Some("Islam".into());
    uid
}

#[test]
fn three_missionaries_in_reach_draw_the_war_and_die_that_turn() {
    let (mut g, mut ai) = fixture();
    let soldiers = [(4, 8), (5, 9), (6, 8)].map(|at| soldier(&mut g, at));
    let targets = [(4, 9), (6, 9), (7, 8)].map(|at| spreader(&mut g, "missionary", at));
    assert!(ai.faith_strike_opening(&mut g, 0));
    assert!(g.is_at_war(0, 1));
    for target in targets {
        assert!(
            !g.units.contains_key(&target),
            "every spreader is condemned"
        );
    }
    for uid in soldiers {
        assert!(g.units.contains_key(&uid));
    }
    assert_eq!(
        g.players[0].counters.get("faith_strike:condemned"),
        Some(&3)
    );
    assert_eq!(
        ai.faith_war_before_the_flip_war.map(|war| war.rival),
        Some(1),
        "the strike runs as the faith war, so peace follows once it is done"
    );
}

#[test]
fn an_apostle_and_a_missionary_are_worth_a_strike() {
    let (mut g, mut ai) = fixture();
    soldier(&mut g, (4, 8));
    soldier(&mut g, (6, 8));
    let apostle = spreader(&mut g, "apostle", (4, 9));
    let missionary = spreader(&mut g, "missionary", (6, 9));
    assert!(ai.faith_strike_opening(&mut g, 0));
    assert!(!g.units.contains_key(&apostle));
    assert!(!g.units.contains_key(&missionary));
}

#[test]
fn two_missionaries_wait_for_a_better_turn() {
    let (mut g, mut ai) = fixture();
    soldier(&mut g, (4, 8));
    soldier(&mut g, (6, 8));
    let first = spreader(&mut g, "missionary", (4, 9));
    let second = spreader(&mut g, "missionary", (6, 9));
    assert!(!ai.faith_strike_opening(&mut g, 0));
    assert!(!g.is_at_war(0, 1), "no war for fewer than the bar");
    assert!(g.units.contains_key(&first) && g.units.contains_key(&second));
}

#[test]
fn one_soldier_cannot_condemn_three() {
    let (mut g, mut ai) = fixture();
    soldier(&mut g, (6, 8));
    for at in [(6, 9), (7, 8), (5, 8)] {
        spreader(&mut g, "missionary", at);
    }
    assert!(!ai.faith_strike_opening(&mut g, 0));
    assert!(!g.is_at_war(0, 1), "kills are matched one soldier each");
}

#[test]
fn the_match_moves_a_soldier_to_free_the_apostle() {
    // The Apostle can only be reached by the first soldier; the Missionary by
    // either. A greedy pass that gives the Missionary the first soldier
    // strands the Apostle.
    let (mut g, mut ai) = fixture();
    soldier(&mut g, (6, 8));
    soldier(&mut g, (9, 8));
    let apostle = spreader(&mut g, "apostle", (5, 8));
    let missionary = spreader(&mut g, "missionary", (8, 8));
    assert!(ai.faith_strike_opening(&mut g, 0));
    assert!(!g.units.contains_key(&apostle));
    assert!(!g.units.contains_key(&missionary));
}

#[test]
fn a_stronger_founder_is_left_alone() {
    let (mut g, mut ai) = fixture();
    for at in [(4, 8), (5, 9), (6, 8)] {
        soldier(&mut g, at);
    }
    for at in [(4, 9), (6, 9), (7, 8)] {
        spreader(&mut g, "missionary", at);
    }
    for x in 26..34 {
        for y in [6, 10] {
            g.spawn_test_unit("spearman", 1, (x, y));
        }
    }
    assert!(!ai.faith_strike_opening(&mut g, 0));
    assert!(!g.is_at_war(0, 1), "the edge gate holds the war");
}

/// A strike in reach, and rival 1 raised past the faith war's edge to parity
/// with spearmen far from us.
fn parity_fixture() -> (Game, AdvancedAi) {
    let (mut g, mut ai) = fixture();
    ai.faith_strike_at_a_quiet_parity = true;
    for at in [(4, 8), (5, 9), (6, 8)] {
        soldier(&mut g, at);
    }
    for at in [(4, 9), (6, 9), (7, 8)] {
        spreader(&mut g, "missionary", at);
    }
    for x in 26..34 {
        if g.military_power(1) > 0.7 * g.military_power(0) {
            break;
        }
        g.spawn_test_unit("spearman", 1, (x, 6));
    }
    let ratio = g.military_power(1) / g.military_power(0);
    assert!(ratio > 0.7 && ratio <= 1.0, "ratio {ratio}");
    (g, ai)
}

#[test]
fn a_rival_at_parity_with_its_army_away_is_struck() {
    let (mut g, mut ai) = parity_fixture();
    assert!(ai.faith_strike_opening(&mut g, 0));
    assert!(g.is_at_war(0, 1));
}

#[test]
fn parity_is_not_enough_without_the_gene() {
    let (mut g, mut ai) = parity_fixture();
    ai.faith_strike_at_a_quiet_parity = false;
    assert!(!ai.faith_strike_opening(&mut g, 0));
    assert!(!g.is_at_war(0, 1));
}

#[test]
fn a_rival_soldier_near_our_cities_breaks_the_quiet() {
    let (mut g, mut ai) = parity_fixture();
    let far = g
        .player_unit_ids(1)
        .into_iter()
        .find(|uid| g.units[uid].kind.as_str() == "spearman")
        .unwrap();
    g.units.get_mut(&far).unwrap().pos = (2, 2);
    assert!(!ai.faith_strike_at_a_quiet_parity_clear(&g));
    assert!(!ai.faith_strike_opening(&mut g, 0));
    assert!(!g.is_at_war(0, 1));
}

#[test]
fn a_rival_scout_near_our_cities_does_not_break_the_quiet() {
    let (mut g, mut ai) = parity_fixture();
    // The scout adds to the rival's power, so we match it to stay at parity.
    soldier(&mut g, (0, 12));
    g.spawn_test_unit("scout", 1, (2, 3));
    let ratio = g.military_power(1) / g.military_power(0);
    assert!(ratio > 0.7 && ratio <= 1.0, "ratio {ratio}");
    assert!(ai.faith_strike_at_a_quiet_parity_clear(&g));
    assert!(ai.faith_strike_opening(&mut g, 0));
}

#[test]
fn a_rival_stronger_than_us_is_left_alone_however_quiet() {
    let (mut g, mut ai) = parity_fixture();
    g.spawn_test_unit("spearman", 1, (33, 10));
    assert!(g.military_power(1) > g.military_power(0));
    assert!(!ai.faith_strike_opening(&mut g, 0));
    assert!(!g.is_at_war(0, 1));
}

impl AdvancedAi {
    /// The quiet-parity read for rival 1 against seat 0, at the current power.
    fn faith_strike_at_a_quiet_parity_clear(&self, g: &Game) -> bool {
        let steady = self.steady_rival_power(g, 1).max(1.0);
        self.faith_strike_quiet_parity(g, 0, 1, steady, g.military_power(0))
    }
}

#[test]
fn a_faith_that_does_not_touch_us_is_no_reason() {
    let (mut g, mut ai) = fixture();
    ai.victory_target = Some(VictoryTarget::Science);
    for at in [(4, 8), (5, 9), (6, 8)] {
        soldier(&mut g, at);
    }
    for at in [(4, 9), (6, 9), (7, 8)] {
        spreader(&mut g, "missionary", at);
    }
    assert!(!ai.faith_strike_opening(&mut g, 0));
    assert!(!g.is_at_war(0, 1));
}

#[test]
fn a_rival_is_not_struck_twice_inside_the_cooldown() {
    let (mut g, mut ai) = fixture();
    for at in [(4, 8), (5, 9), (6, 8)] {
        soldier(&mut g, at);
    }
    for at in [(4, 9), (6, 9), (7, 8)] {
        spreader(&mut g, "missionary", at);
    }
    ai.faith_strike_last.insert(1, 80);
    assert!(!ai.faith_strike_opening(&mut g, 0));
    assert!(!g.is_at_war(0, 1));
    g.turn = 80 + g.standard_duration(FAITH_STRIKE_COOLDOWN_TURNS);
    assert!(ai.faith_strike_opening(&mut g, 0));
    assert!(g.is_at_war(0, 1));
}

#[test]
fn off_nothing_changes() {
    let (mut g, mut ai) = fixture();
    ai.faith_strike_wipes_the_spreaders = false;
    for at in [(4, 8), (5, 9), (6, 8)] {
        soldier(&mut g, at);
    }
    for at in [(4, 9), (6, 9), (7, 8)] {
        spreader(&mut g, "missionary", at);
    }
    assert!(!ai.faith_strike_opening(&mut g, 0));
    assert!(!g.is_at_war(0, 1));
}

#[test]
fn the_diplomacy_step_opens_the_strike() {
    let (mut g, mut ai) = fixture();
    for at in [(4, 8), (5, 9), (6, 8)] {
        soldier(&mut g, at);
    }
    let targets = [(4, 9), (6, 9), (7, 8)].map(|at| spreader(&mut g, "missionary", at));
    let plan = StrategicPlan {
        strategy: GrandStrategy::Religion,
        target_player: None,
        target_city: None,
        threatened_city: None,
        desired_cities: 3,
        assessed_turn: g.turn,
        rush: false,
    };
    ai.advanced_diplomacy(&mut g, 0, &plan);
    assert!(g.is_at_war(0, 1));
    assert!(targets.iter().all(|uid| !g.units.contains_key(uid)));
}

/// `faith-strike-spares-the-stages`: a Culture seat whose city is being
/// converted to the rival faith (at 70% of the city's strongest pressure),
/// with three Missionaries in reach of three soldiers.
fn culture_fixture() -> (Game, AdvancedAi) {
    let (mut g, mut ai) = fixture();
    ai.victory_target = Some(VictoryTarget::Culture);
    let home = g.player_city_ids(0)[0];
    let pressure = &mut g.cities.get_mut(&home).unwrap().pressure;
    pressure.insert("Buddhism".into(), 100.0);
    pressure.insert("Islam".into(), 70.0);
    for at in [(4, 8), (5, 9), (6, 8)] {
        soldier(&mut g, at);
    }
    for at in [(4, 9), (6, 9), (7, 8)] {
        spreader(&mut g, "missionary", at);
    }
    (g, ai)
}

#[test]
fn the_stages_gene_ships_off_and_is_registered() {
    super::super::test_support::opt_in_off_in_both_controllers(
        "faith-strike-spares-the-stages",
        |ai| ai.faith_strike_spares_the_stages,
    );
}

#[test]
fn cold_war_holds_the_strike_on_a_culture_seat() {
    let (g, mut ai) = culture_fixture();
    ai.enable_faith_strike_spares_the_stages();
    let mut before = g.clone();
    assert!(
        ai.faith_strike_opening(&mut before, 0),
        "before the bands the strike goes"
    );

    let (mut g, mut ai) = culture_fixture();
    g.players[0].civics.insert(crate::name!("cold_war"));
    let mut off = g.clone();
    assert!(ai.faith_strike_opening(&mut off, 0), "off: struck");
    ai.enable_faith_strike_spares_the_stages();
    assert_eq!(
        ai.faith_strike_gate(&g, 0, 1).map(|(gate, _)| gate),
        Some("stages")
    );
    assert!(!ai.faith_strike_opening(&mut g, 0));
    assert!(!g.is_at_war(0, 1), "the stages hold the strike");

    let (mut g, mut ai) = culture_fixture();
    g.players[0].civics.insert(crate::name!("cold_war"));
    ai.enable_faith_strike_spares_the_stages();
    ai.victory_target = Some(VictoryTarget::Science);
    assert!(
        ai.faith_strike_opening(&mut g, 0),
        "another target strikes as before"
    );
}

#[test]
fn an_open_band_bank_holds_the_strike() {
    let (mut g, mut ai) = culture_fixture();
    ai.enable_faith_strike_spares_the_stages();
    g.players[0].civics.insert(crate::name!("nationalism"));
    let mut shut = g.clone();
    assert!(
        ai.faith_strike_opening(&mut shut, 0),
        "no bank gene: the bank is shut and the strike goes"
    );
    ai.enable_culture_bank_opens_at_nationalism();
    assert!(ai.culture_band_bank(&g, 0) > 0.0);
    assert!(!ai.faith_strike_opening(&mut g, 0));
    assert!(!g.is_at_war(0, 1));
}

/// The heretic strike's war on rival 1, opened at turn 83, with our city
/// still being converted and Cold War known.
fn strike_war() -> (Game, AdvancedAi, StrategicPlan) {
    let (mut g, mut ai) = culture_fixture();
    g.players[0].civics.insert(crate::name!("cold_war"));
    g.at_war.insert((0, 1));
    g.at_war.insert((1, 0));
    ai.faith_strike_last.insert(1, 83);
    ai.faith_war_before_the_flip_war = Some(faith_war_before_the_flip::FaithWar {
        rival: 1,
        opened: 83,
        spreader_seen: 83,
    });
    g.turn = 95;
    let plan = StrategicPlan {
        strategy: GrandStrategy::Culture,
        target_player: None,
        target_city: None,
        threatened_city: None,
        desired_cities: 3,
        assessed_turn: g.turn,
        rush: false,
    };
    (g, ai, plan)
}

#[test]
fn the_strike_war_ends_once_the_bands_need_the_stages() {
    let (g, mut ai, plan) = strike_war();
    assert!(ai.faith_war_keeps_the_war(&g, 0, 1), "off: kept");
    assert!(!ai.faith_war_exit_peace(&g, 0, 1, &plan), "off: no exit");
    ai.enable_faith_strike_spares_the_stages();
    assert!(!ai.faith_war_keeps_the_war(&g, 0, 1));
    assert!(ai.faith_war_exit_peace(&g, 0, 1, &plan));

    let mut targeted = plan.clone();
    targeted.target_player = Some(1);
    assert!(
        !ai.faith_war_exit_peace(&g, 0, 1, &targeted),
        "the plan's own war is not ended"
    );

    let (mut g, mut ai, plan) = strike_war();
    ai.enable_faith_strike_spares_the_stages();
    g.players[0].civics.remove(&crate::name!("cold_war"));
    assert!(ai.faith_war_keeps_the_war(&g, 0, 1), "before the bands");
    assert!(!ai.faith_war_exit_peace(&g, 0, 1, &plan));

    let (g, mut ai, plan) = strike_war();
    ai.enable_faith_strike_spares_the_stages();
    ai.faith_strike_last.clear();
    assert!(
        ai.faith_war_keeps_the_war(&g, 0, 1),
        "a faith war the strike did not open is unchanged"
    );
    assert!(!ai.faith_war_exit_peace(&g, 0, 1, &plan));
}

/// Rival 1 knows `lead` more techs than we do, on the seats' own trees.
fn rival_leads_by(g: &mut Game, lead: usize) {
    let target = g.players[0].techs.len() + lead;
    let unknown: Vec<_> = g
        .rules
        .techs
        .keys()
        .copied()
        .filter(|tech| !g.players[1].techs.contains(tech))
        .collect();
    let missing = target.saturating_sub(g.players[1].techs.len());
    g.players[1].techs.extend(unknown.into_iter().take(missing));
    assert_eq!(g.players[1].techs.len(), target);
}

#[test]
fn the_tech_lead_gene_ships_off_and_is_registered() {
    super::super::test_support::opt_in_off_in_both_controllers(
        "faith-strike-fears-the-tech-lead",
        |ai| ai.faith_strike_fears_the_tech_lead,
    );
}

#[test]
fn a_rival_far_ahead_in_technology_is_not_struck() {
    let (mut g, mut ai) = culture_fixture();
    rival_leads_by(&mut g, FAITH_STRIKE_TECH_LEAD);
    let mut off = g.clone();
    assert!(ai.faith_strike_opening(&mut off, 0), "off: struck");
    ai.enable_faith_strike_fears_the_tech_lead();
    let (gate, reason) = ai.faith_strike_gate(&g, 0, 1).unwrap();
    assert_eq!(gate, "tech_lead");
    assert!(reason.starts_with(&format!("it leads us by {FAITH_STRIKE_TECH_LEAD} techs")));
    assert!(!ai.faith_strike_opening(&mut g, 0));
    assert!(!g.is_at_war(0, 1));

    let (mut g, mut ai) = culture_fixture();
    rival_leads_by(&mut g, FAITH_STRIKE_TECH_LEAD - 1);
    ai.enable_faith_strike_fears_the_tech_lead();
    assert!(ai.faith_strike_opening(&mut g, 0), "one short of the bar");

    let (mut g, mut ai) = culture_fixture();
    rival_leads_by(&mut g, FAITH_STRIKE_TECH_LEAD);
    ai.enable_faith_strike_fears_the_tech_lead();
    ai.victory_target = Some(VictoryTarget::Religion);
    assert!(
        ai.faith_strike_opening(&mut g, 0),
        "a Religion seat strikes as before"
    );
}

/// A live board redacts a rival's tree; the host's public count is read.
#[test]
fn the_tech_lead_reads_the_hosts_public_count() {
    let (mut g, mut ai) = culture_fixture();
    ai.enable_faith_strike_fears_the_tech_lead();
    assert_eq!(ai.faith_strike_gate(&g, 0, 1), None);
    let ours = g.players[0].techs.len();
    std::sync::Arc::make_mut(&mut g.observed_public_empire_stats)
        .entry(1)
        .or_default()
        .techs = Some(ours + FAITH_STRIKE_TECH_LEAD);
    assert_eq!(
        ai.faith_strike_gate(&g, 0, 1).map(|(gate, _)| gate),
        Some("tech_lead")
    );
    assert!(!ai.faith_strike_opening(&mut g, 0));
}
