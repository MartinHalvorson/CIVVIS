use super::*;
use crate::game::Item;
use crate::name;

/// Two founded empires at turn 110 (Online-speed midgame), in contact, with
/// a Domination seat as player 0 planning a conquest of player 1.
fn board(civ: &str) -> (Game, AdvancedAi, StrategicPlan) {
    let mut g = Game::new_full(2, 30, 20, 109_104_000, 300, 0, false);
    for pid in 0..2 {
        let settler = g
            .player_unit_ids(pid)
            .into_iter()
            .find(|id| g.units[id].kind == "settler")
            .unwrap();
        g.found_city_for(pid, g.units[&settler].pos, None);
        // A second city each, so the empire-size gate passes.
        let home = g.cities[&g.player_city_ids(pid)[0]].pos;
        let second = g
            .wdisk(home, 6)
            .into_iter()
            .find(|pos| {
                g.wdist(*pos, home) >= 4
                    && !g.rules.is_water(&g.map.tiles[pos])
                    && g.units_at(*pos).is_empty()
                    && g.cities.values().all(|city| g.wdist(city.pos, *pos) >= 4)
            })
            .unwrap();
        g.found_city_for(pid, second, None);
    }
    g.current = 0;
    g.turn = 110;
    g.record_contact(0, 1);
    g.players[0].civ = civ.to_string();
    g.players[1].civ = "Egypt".to_string();
    let mut ai = AdvancedAi::targeting(VictoryTarget::Domination);
    ai.enable_decisive_window();
    ai.battlefront_observation = false;
    let plan = StrategicPlan {
        strategy: GrandStrategy::Conquest,
        target_player: Some(1),
        target_city: Some(g.player_city_ids(1)[0]),
        threatened_city: None,
        desired_cities: 3,
        assessed_turn: g.turn,
        rush: false,
    };
    (g, ai, plan)
}

fn learn(g: &mut Game, pid: usize, techs: &[&str]) {
    for tech in techs {
        let ancestors = g.rules.tech_ancestors[*tech].clone();
        g.players[pid]
            .techs
            .extend(ancestors.iter().map(|tech| Name::new(tech)));
        g.players[pid].techs.insert(Name::new(tech));
    }
}

/// Quote every unknown technology at five beakers, as a host snapshot would
/// for a seat that is nearly there, so the research horizon reads as met.
fn quote_cheap(g: &mut Game, pid: usize) {
    let unknown: Vec<Name> = g
        .rules
        .techs
        .keys()
        .copied()
        .filter(|tech| !g.players[pid].techs.contains(tech))
        .collect();
    let quotes = std::sync::Arc::make_mut(&mut g.host_research_quotes)
        .entry(pid)
        .or_default();
    for tech in unknown {
        quotes.insert(
            tech,
            crate::game::HostResearchQuote {
                cost: Some(5.0),
                progress: Some(0.0),
            },
        );
    }
}

fn learn_civics(g: &mut Game, pid: usize, civics: &[&str]) {
    for civic in civics {
        let ancestors = g.rules.civic_ancestors[*civic].clone();
        g.players[pid]
            .civics
            .extend(ancestors.iter().map(|civic| Name::new(civic)));
        g.players[pid].civics.insert(Name::new(civic));
    }
}

/// The live Gran Colombia shape: the rival stands behind Siege Tactics
/// walls with Musketmen and Pikemen; we hold the Medieval tree. The window
/// is the Llanero (our unique unit) with a Bombard, so research goes to the
/// Bombard's Metal Casting or the Llanero's Military Science — and the
/// research chooser takes a step toward one of them.
#[test]
fn gran_colombia_researches_toward_the_llanero_and_a_bombard() {
    let (mut g, ai, plan) = board("Gran Colombia");
    learn(&mut g, 1, &["siege_tactics", "gunpowder", "military_tactics"]);
    learn(&mut g, 0, &["printing", "castles", "gunpowder", "animal_husbandry"]);
    g.players[0]
        .strategic_resources
        .insert(name!("horses"), 40.0);
    g.players[0]
        .strategic_resources
        .insert(name!("niter"), 40.0);
    let window = ai.decisive_window_within(&g, 0, &plan, f64::INFINITY).expect("a window");
    assert_eq!(window.target, 1);
    assert_eq!(window.assault, name!("llanero"));
    assert!(window.unique);
    assert_eq!(window.wall_tier, 3);
    assert_eq!(window.breaker, Some(name!("bombard")));
    assert!(window.margin >= DECISIVE_MARGIN, "{window:?}");
    let goal = window.tech_goal.expect("research is still owed");
    assert!(
        [name!("metal_casting"), name!("military_science")].contains(&goal),
        "{goal:?}"
    );
    let mut g = g;
    quote_cheap(&mut g, 0);
    assert!(ai.decisive_window(&g, 0, &plan).is_some_and(|w| w.tech_goal.is_some()));
    g.players[0].research = None;
    ai.advanced_research(&mut g, 0, &plan);
    let picked = g.players[0].research.clone().expect("a research pick");
    assert!(
        ai.tech_leads_to(&g, &picked, "metal_casting")
            || ai.tech_leads_to(&g, &picked, "military_science"),
        "{picked:?} is not on the window's path"
    );
}

/// `siege-tier-yields-to-the-bombers`: the Llanero-and-Bombard board with
/// Education in hand, so the air surge's pre-plan step (Industrialization)
/// has a goal. Gene off, the window keeps the slot for Metal Casting or
/// Military Science; gene on, it yields and research steps toward the
/// Bomber instead.
#[test]
fn the_window_yields_an_off_path_goal_to_the_bombers_under_the_gene() {
    let (mut g, mut ai, plan) = board("Gran Colombia");
    learn(&mut g, 1, &["siege_tactics", "gunpowder", "military_tactics"]);
    learn(
        &mut g,
        0,
        &["printing", "castles", "gunpowder", "animal_husbandry", "education"],
    );
    g.players[0]
        .strategic_resources
        .insert(name!("horses"), 40.0);
    g.players[0]
        .strategic_resources
        .insert(name!("niter"), 40.0);
    quote_cheap(&mut g, 0);
    ai.enable_air_surge_2();
    let window = ai.decisive_window(&g, 0, &plan).expect("a window");
    let goal = window.tech_goal.expect("research is still owed");
    assert!(
        [name!("metal_casting"), name!("military_science")].contains(&goal),
        "{goal:?}"
    );
    // Gene off: the window's goal holds; Ballistics is still the surge's gate.
    assert!(!ai.window_yields_to_the_bombers(&g, 0, goal));
    assert_eq!(ai.decisive_window_research_goal(&g, 0, Some(&window)), Some(goal));
    assert_eq!(ai.air_surge_research_goal(&g, 0), None);

    ai.enable_siege_tier_yields_to_the_bombers();
    assert_eq!(ai.air_surge_research_goal(&g, 0), Some("industrialization"));
    assert!(ai.window_yields_to_the_bombers(&g, 0, goal));
    assert_eq!(ai.decisive_window_research_goal(&g, 0, Some(&window)), None);
    // A goal on the Advanced Flight path never yields.
    assert!(!ai.window_yields_to_the_bombers(&g, 0, name!("industrialization")));
    assert!(!ai.window_yields_to_the_bombers(&g, 0, name!("advanced_flight")));
    g.players[0].research = None;
    ai.advanced_research(&mut g, 0, &plan);
    let picked = g.players[0].research.clone().expect("a research pick");
    assert!(
        ai.tech_leads_to(&g, &picked, "industrialization"),
        "{picked:?} is not on the way to the Bomber"
    );
}

/// The same board with both halves unlocked: the window is open, there is
/// nothing to research for it, and every other lane keeps the slot.
#[test]
fn an_unlocked_package_is_an_open_window_and_yields_research() {
    let (mut g, ai, plan) = board("Gran Colombia");
    learn(&mut g, 1, &["siege_tactics", "gunpowder", "military_tactics"]);
    learn(
        &mut g,
        0,
        &["military_science", "metal_casting", "animal_husbandry"],
    );
    g.players[0]
        .strategic_resources
        .insert(name!("horses"), 40.0);
    g.players[0]
        .strategic_resources
        .insert(name!("niter"), 40.0);
    let window = ai.decisive_window_within(&g, 0, &plan, f64::INFINITY).expect("a window");
    assert!(window.open(), "{window:?}");
    assert_eq!(window.turns, 0.0);
    assert_eq!(ai.decisive_window_tech_goal(&g, 0, &plan), None);
    // An open window needs no research, so the horizon cannot hide it.
    assert_eq!(ai.decisive_window(&g, 0, &plan), Some(window));
}

/// A rival one step from Castles (Construction and Masonry held) is planned
/// against Medieval walls, where a Battering Ram no longer works: a Rome
/// seat's package carries a Siege Tower or a gun, never a ram.
#[test]
fn rams_do_not_count_against_medieval_walls() {
    let (mut g, ai, plan) = board("Rome");
    learn(&mut g, 1, &["construction", "masonry", "bronze_working", "archery"]);
    learn(&mut g, 0, &["masonry", "iron_working", "bronze_working"]);
    g.players[0]
        .strategic_resources
        .insert(name!("iron"), 40.0);
    assert_eq!(ai.decisive_wall_tier(&g, 1), (1, 2));
    let window = ai.decisive_window_within(&g, 0, &plan, f64::INFINITY).expect("a window");
    assert_eq!(window.wall_tier, 2);
    assert_ne!(window.breaker, Some(name!("battering_ram")), "{window:?}");
    assert!(window.breaker.is_some());
    // Without the Construction step Castles is out of reach: Ancient walls,
    // and the ram is back in play.
    g.players[1].techs.remove(&name!("construction"));
    assert_eq!(ai.decisive_wall_tier(&g, 1), (1, 1));
}

/// A civic-gated unique unit drives the civic chooser: Japan's Samurai is
/// Feudalism, not a technology.
#[test]
fn japan_beelines_feudalism_for_the_samurai() {
    let (mut g, mut ai, mut plan) = board("Japan");
    // An unwalled rival with Swordsmen: the Samurai (48+5) is decisive and,
    // at three quarters of its price, the cheapest package.
    learn(&mut g, 1, &["iron_working"]);
    learn(&mut g, 0, &["iron_working", "bronze_working"]);
    learn_civics(&mut g, 0, &["military_training"]);
    g.players[0]
        .strategic_resources
        .insert(name!("iron"), 40.0);
    let window = ai.decisive_window_within(&g, 0, &plan, f64::INFINITY).expect("a window");
    if window.assault == name!("samurai") {
        assert_eq!(window.civic_goal, Some(name!("feudalism")));
        assert_eq!(
            ai.decisive_window_within(&g, 0, &plan, f64::INFINITY)
                .and_then(|window| window.civic_goal),
            Some(name!("feudalism"))
        );
    }
    // Whatever the package, the gene and the plan gate it.
    ai.disable_decisive_window();
    assert_eq!(ai.decisive_window(&g, 0, &plan), None);
    ai.enable_decisive_window();
    plan.strategy = GrandStrategy::Science;
    let mut science = AdvancedAi::targeting(VictoryTarget::Science);
    science.enable_decisive_window();
    science.battlefront_observation = false;
    assert_eq!(science.decisive_window(&g, 0, &plan), None);
}

#[test]
fn the_window_waits_for_the_opening_and_a_second_city() {
    let (mut g, ai, plan) = board("Gran Colombia");
    g.turn = g.standard_duration(DECISIVE_WINDOW_OPENS) - 1;
    assert_eq!(ai.decisive_window(&g, 0, &plan), None);
    g.turn = 110;
    assert!(ai.decisive_window_within(&g, 0, &plan, f64::INFINITY).is_some());
    let second = g.player_city_ids(0)[1];
    g.cities.remove(&second);
    assert_eq!(ai.decisive_window_within(&g, 0, &plan, f64::INFINITY), None);
}

#[test]
fn both_genes_are_opt_in_and_off_by_default() {
    assert!(!AdvancedAi::new().decisive_window);
    assert!(!AdvancedAi::legacy().decisive_window);
    assert!(!AdvancedAi::new().base.unique_unit_preference);
    for tag in ["decisive-window", "unique-unit-preference"] {
        assert!(
            super::super::GENES
                .iter()
                .any(|gene| gene.tag == tag && gene.opt_in()),
            "{tag}"
        );
    }
}

/// `unique-unit-preference`: the Llanero is built over the stronger-on-paper
/// Line Infantry — until it is half of the melee army.
#[test]
fn the_build_picker_prefers_the_llanero_until_half_the_army() {
    let (mut g, mut ai, _) = board("Gran Colombia");
    learn(&mut g, 0, &["military_science", "animal_husbandry"]);
    g.players[0]
        .strategic_resources
        .insert(name!("horses"), 200.0);
    g.players[0]
        .strategic_resources
        .insert(name!("niter"), 200.0);
    let city = g.player_city_ids(0)[0];
    let pos = g.cities[&city].pos;
    for unit in g.player_unit_ids(0) {
        g.remove_unit(unit);
    }
    let stock = ai.base.best_military(&g, 0, city, Some(false));
    assert_eq!(stock.as_deref(), Some("line_infantry"));
    ai.enable_unique_unit_preference();
    assert_eq!(
        ai.base.best_military(&g, 0, city, Some(false)).as_deref(),
        Some("llanero")
    );
    g.spawn_test_unit("llanero", 0, pos);
    g.spawn_test_unit("line_infantry", 0, pos);
    assert_eq!(
        ai.base.best_military(&g, 0, city, Some(false)).as_deref(),
        Some("line_infantry"),
        "one Llanero of two melee bodies is already half"
    );
    g.cities.get_mut(&city).unwrap().queue.push(Item::Unit {
        unit: name!("line_infantry"),
    });
    assert_eq!(
        ai.base.best_military(&g, 0, city, Some(false)).as_deref(),
        Some("llanero")
    );
}

/// With the window's package needing no civic, Nationalism (Corps) is the
/// civic goal once it is inside the horizon — and not before.
#[test]
fn nationalism_is_the_civic_goal_inside_the_horizon() {
    let (mut g, ai, plan) = board("Gran Colombia");
    learn(&mut g, 1, &["siege_tactics", "gunpowder", "military_tactics"]);
    learn(&mut g, 0, &["printing", "castles", "gunpowder", "animal_husbandry"]);
    g.players[0].strategic_resources.insert(name!("horses"), 40.0);
    g.players[0].strategic_resources.insert(name!("niter"), 40.0);
    assert_eq!(
        ai.decisive_window_civic_goal_within(&g, 0, &plan, f64::INFINITY),
        Some(name!("nationalism"))
    );
    assert_eq!(
        ai.decisive_window_civic_goal_within(&g, 0, &plan, 0.0),
        None,
        "outside the horizon nothing is owed"
    );
    learn_civics(&mut g, 0, &["nationalism"]);
    assert_eq!(
        ai.decisive_window_civic_goal_within(&g, 0, &plan, f64::INFINITY),
        None
    );
}

/// `found-against-a-rival-faith`: with a decisive window pending, a rival
/// faith at the early-warning bar (two of four majors) still wins Astrology
/// for a faithless conqueror, and the window's research goal stands down
/// whatever order the two forced-goal arms merge in.
#[test]
fn a_pending_window_yields_astrology_to_the_faith_veto() {
    let mut g = Game::new_full(4, 34, 20, 76_108, 300, 0, false);
    for pid in 0..3 {
        let settler = g
            .player_unit_ids(pid)
            .into_iter()
            .find(|unit| g.units[unit].kind == "settler")
            .unwrap();
        let at = g.units[&settler].pos;
        g.remove_unit(settler);
        g.found_city_for(pid, at, None);
    }
    let home = g.cities[&g.player_city_ids(0)[0]].pos;
    let second = g
        .wdisk(home, 6)
        .into_iter()
        .find(|pos| {
            g.wdist(*pos, home) >= 4
                && !g.rules.is_water(&g.map.tiles[pos])
                && g.units_at(*pos).is_empty()
                && g.cities.values().all(|city| g.wdist(city.pos, *pos) >= 4)
        })
        .unwrap();
    g.found_city_for(0, second, None);
    g.current = 0;
    g.turn = 110;
    g.record_contact(0, 1);
    g.players[0].civ = "Gran Colombia".to_string();
    g.players[1].civ = "Egypt".to_string();
    learn(&mut g, 1, &["siege_tactics", "gunpowder", "military_tactics"]);
    learn(&mut g, 0, &["printing", "castles", "gunpowder", "animal_husbandry"]);
    g.players[0].techs.remove(&name!("astrology"));
    g.players[0].strategic_resources.insert(name!("horses"), 40.0);
    g.players[0].strategic_resources.insert(name!("niter"), 40.0);
    quote_cheap(&mut g, 0);
    let faith = "Rival Faith".to_string();
    g.players[1].religion = Some(faith.clone());
    for pid in [1, 2] {
        for city in g.player_city_ids(pid) {
            g.cities
                .get_mut(&city)
                .unwrap()
                .pressure
                .insert(faith.clone(), 5_000.0);
        }
    }
    let plan = StrategicPlan {
        strategy: GrandStrategy::Conquest,
        target_player: Some(1),
        target_city: Some(g.player_city_ids(1)[0]),
        threatened_city: None,
        desired_cities: 3,
        assessed_turn: g.turn,
        rush: false,
    };
    let mut ai = AdvancedAi::targeting(VictoryTarget::Domination);
    ai.enable_decisive_window();
    ai.battlefront_observation = false;

    let window = ai.decisive_window(&g, 0, &plan).expect("a pending window");
    let goal = window.tech_goal.expect("the window owes research");
    assert_eq!(
        ai.decisive_window_research_goal(&g, 0, Some(&window)),
        Some(goal),
        "without the faith gene the window keeps its goal"
    );
    let mut off = g.clone();
    off.players[0].research = None;
    ai.advanced_research(&mut off, 0, &plan);
    let picked = off.players[0].research.clone().expect("a pick");
    assert!(ai.tech_leads_to(&off, &picked, goal.as_str()), "{picked}");

    ai.enable_found_against_a_rival_faith();
    assert!(ai.faith_veto_due(&g, 0));
    assert_eq!(
        ai.decisive_window_research_goal(&g, 0, Some(&window)),
        None,
        "the faith veto takes the slot from the window"
    );
    assert!(ai.prophet_race_enterable_for(&g, 0, Some(VictoryTarget::Domination)));
    let mut on = g.clone();
    on.players[0].research = None;
    ai.advanced_research(&mut on, 0, &plan);
    assert_eq!(
        on.players[0].research.as_deref(),
        Some("astrology"),
        "a pending window still lets the faith Astrology win"
    );

    // Once Astrology is known the veto has nothing left to research, and the
    // window takes the slot back although `faith_veto_due` still holds (live
    // G80 yielded four more times after its t35 Astrology).
    let mut known = g.clone();
    known.players[0].techs.insert(name!("astrology"));
    assert!(ai.faith_veto_due(&known, 0), "the veto itself stays due");
    let window = ai.decisive_window(&known, 0, &plan).expect("still pending");
    assert_eq!(
        ai.decisive_window_research_goal(&known, 0, Some(&window)),
        window.tech_goal,
        "the window resumes once Astrology is known"
    );
    known.players[0].research = None;
    ai.advanced_research(&mut known, 0, &plan);
    let picked = known.players[0].research.clone().expect("a pick");
    let goal = window.tech_goal.expect("a goal");
    assert!(ai.tech_leads_to(&known, &picked, goal.as_str()), "{picked}");
}

/// Numbers relax the margin: against defenders that outclass our army, an
/// even fight needs a better assault, but at `OVERWHELMING_POWER` the
/// package is our existing assault plus the breaker that opens the walls
/// (live G94: 5-8x Portugal's power, "no package" t73-t135, Trebuchets at
/// t111 for walls raised at t86).
#[test]
fn overwhelming_power_reduces_the_window_to_a_breaker() {
    let (mut g, ai, plan) = board("Gran Colombia");
    // Medieval walls are one step away for the target; its Pikemen and
    // Men-at-Arms outclass our Swordsmen.
    learn(&mut g, 1, &["construction", "masonry", "military_tactics", "apprenticeship"]);
    learn(&mut g, 0, &["iron_working", "bronze_working", "masonry"]);
    g.players[0].strategic_resources.insert(name!("iron"), 40.0);
    let even = ai
        .decisive_window_within(&g, 0, &plan, f64::INFINITY)
        .expect("some package eventually outclasses them");
    assert!(even.margin >= DECISIVE_MARGIN, "{even:?}");
    assert!(even.power_ratio < DOMINANT_POWER, "{even:?}");

    let home = g.cities[&g.player_city_ids(0)[0]].pos;
    for _ in 0..12 {
        g.spawn_test_unit("swordsman", 0, home);
    }
    let crushing = ai
        .decisive_window_within(&g, 0, &plan, f64::INFINITY)
        .expect("a breaker package");
    assert!(crushing.power_ratio >= OVERWHELMING_POWER, "{crushing:?}");
    let assault = &g.rules.units[crushing.assault];
    assert!(
        assault.tech.is_none_or(|tech| g.players[0].techs.contains(&tech)),
        "the assault is one we already field: {crushing:?}"
    );
    let breaker = crushing.breaker.expect("walls need a breaker");
    let breaker_tech = g.rules.units[breaker].tech.expect("a researched breaker");
    let goal = crushing.tech_goal.expect("the breaker is still owed");
    assert!(
        ai.tech_leads_to(&g, goal.as_str(), breaker_tech.as_str()),
        "{goal} leads to {breaker_tech}: {crushing:?}"
    );
    assert!(crushing.turns <= even.turns, "{crushing:?} vs {even:?}");
}

/// An unlocked Bomber opens Urban Defenses as the air surge's four-plane
/// wing: against a 115-strength city two Bombers fall short of 400 walls but
/// four do not, so a crushing army holding Bombers has an open window rather
/// than a Jet Bomber chase (live G104 researched toward Stealth Technology
/// at t177 with Bombers in hand since t158).
#[test]
fn an_unlocked_bomber_wing_opens_urban_defenses() {
    let (mut g, ai, plan) = board("Gran Colombia");
    learn(&mut g, 1, &["steel"]);
    let target = plan.target_city.unwrap();
    std::sync::Arc::make_mut(&mut g.observed_city_strength).insert(target, 115.0);
    learn(&mut g, 0, &["advanced_flight", "military_science", "animal_husbandry"]);
    for resource in ["horses", "aluminum", "niter", "oil"] {
        g.players[0].strategic_resources.insert(Name::new(resource), 60.0);
    }
    let home = g.cities[&g.player_city_ids(0)[0]].pos;
    for _ in 0..12 {
        g.spawn_test_unit("line_infantry", 0, home);
    }
    let window = ai
        .decisive_window_within(&g, 0, &plan, f64::INFINITY)
        .expect("a window");
    assert_eq!(window.wall_tier, 4, "{window:?}");
    assert!(window.power_ratio >= OVERWHELMING_POWER, "{window:?}");
    assert_eq!(window.breaker, Some(name!("bomber")), "{window:?}");
    assert!(window.open(), "nothing left to research: {window:?}");
}

/// Research and builds agree: the readiness pass builds the air surge's
/// whole wing when the campaign target stands behind Urban Defenses, the
/// case the window sizes Bombers as that wing, and keeps its launch pair
/// otherwise or with the gene off (live G104 stopped at two Bombers).
#[test]
fn readiness_builds_the_wing_behind_urban_defenses() {
    use super::super::air_surge::{AIR_SURGE_BOMBERS, AIR_SURGE_LAUNCH_BOMBERS};
    let (mut g, mut ai, plan) = board("Gran Colombia");
    ai.plan = Some(plan.clone());
    learn(&mut g, 1, &["castles"]);
    assert_eq!(ai.decisive_air_wing_bombers(&g, 0), AIR_SURGE_LAUNCH_BOMBERS);
    learn(&mut g, 1, &["steel"]);
    assert_eq!(ai.decisive_air_wing_bombers(&g, 0), AIR_SURGE_BOMBERS);
    ai.disable_decisive_window();
    assert_eq!(ai.decisive_air_wing_bombers(&g, 0), AIR_SURGE_LAUNCH_BOMBERS);
}

/// `breaker-research-first`: an unmined Niter deposit is no Bombard. With
/// neither stock nor income the window prices another breaker, and the
/// Bombard comes back once Niter is in hand (live G136: Military Engineering
/// at t87 for a Bombard, no Niter income until t142).
#[test]
fn breaker_research_first_needs_the_resource_in_hand() {
    let (mut g, mut ai, plan) = board("Gran Colombia");
    learn(&mut g, 1, &["siege_tactics", "gunpowder", "military_tactics"]);
    learn(
        &mut g,
        0,
        &["printing", "castles", "gunpowder", "animal_husbandry", "military_engineering"],
    );
    g.players[0].strategic_resources.insert(name!("horses"), 40.0);
    let capital = g.player_city_ids(0)[0];
    let centre = g.cities[&capital].pos;
    let deposit = *g.cities[&capital]
        .owned_tiles
        .iter()
        .find(|pos| **pos != centre)
        .expect("a worked tile");
    g.map.tiles.get_mut(&deposit).unwrap().resource = Some(name!("niter"));
    assert_eq!(g.strategic_stockpile(0, name!("niter")), 0.0);
    assert_eq!(g.strategic_resource_rate(0, "niter"), 0.0);

    let off = ai
        .decisive_window_within(&g, 0, &plan, f64::INFINITY)
        .expect("a window");
    assert_eq!(off.breaker, Some(name!("bombard")), "{off:?}");
    ai.enable_breaker_research_first();
    let on = ai
        .decisive_window_within(&g, 0, &plan, f64::INFINITY)
        .expect("a window");
    assert_ne!(on.breaker, Some(name!("bombard")), "{on:?}");

    g.players[0].strategic_resources.insert(name!("niter"), 20.0);
    let supplied = ai
        .decisive_window_within(&g, 0, &plan, f64::INFINITY)
        .expect("a window");
    assert_eq!(supplied.breaker, Some(name!("bombard")), "{supplied:?}");
}

/// `breaker-research-first`: while a siege is held for its wall-breaker,
/// "modernize the standing army" does not take a technology that unlocks no
/// breaker (live G136: Gunpowder, Metal Casting's path and Ballistics at
/// t93-t103 while Victoria waited 77 turns), and a breaker technology is
/// never blocked.
#[test]
fn modernization_yields_while_a_siege_waits_for_its_breaker() {
    let (g, mut ai, plan) = board("Gran Colombia");
    let city = g.cities[&plan.target_city.unwrap()].pos;
    let musket = name!("gunpowder");
    let bombard = name!("metal_casting");
    assert!(!ai.modernization_yields_to_the_breaker(&g, 0, musket), "gene off");
    ai.enable_breaker_research_first();
    assert!(!ai.modernization_yields_to_the_breaker(&g, 0, musket), "no siege held");
    ai.siege_breaker_waits.insert(
        city,
        super::super::siege_train::BreakerWait {
            since: g.turn - 3,
            last: g.turn,
            nearest: 12,
            nearest_turn: g.turn - 3,
        },
    );
    assert!(ai.modernization_yields_to_the_breaker(&g, 0, musket));
    assert!(!ai.modernization_yields_to_the_breaker(&g, 0, bombard), "a breaker tech");
    ai.siege_breaker_waits.get_mut(&city).unwrap().last = g.turn - 5;
    assert!(!ai.modernization_yields_to_the_breaker(&g, 0, musket), "a stale hold");
}
