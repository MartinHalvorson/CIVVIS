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
