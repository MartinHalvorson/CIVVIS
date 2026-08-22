//! Where the research economy actually stops, counted rather than inferred.
//!
//! Four price genes (`advanced/science_scaling.rs`) each moved technology by
//! a tenth of a tech against ~62, on a win axis that resolves ±13.9 pp. That
//! is consistent with two very different worlds, and the answer changes what
//! to build next:
//!
//! 1. the prices move and the FUNNEL does not — the bottleneck is not price,
//!    and no amount of repricing the Campus chain will matter; or
//! 2. the funnel moves and the technology does not — science is a smaller
//!    lever in this model than the live seat's 41-against-112 suggests.
//!
//! The screen's rows carry `techs` and not buildings, so neither can be read
//! off them. This counts the chain directly: Campus, Library, University and
//! Research Lab held at the end, per city, treated against control on the
//! same seeds and the same map shape the screen runs.
//!
//! ⚠ A census, not an assertion. It plays whole games and is `#[ignore]`d.
//!
//! ⚠ Adding a gene makes `HEURISTIC_GENE_RANKING.md` stale — it is generated
//! and lists every screenable gene, so `tools/test_heuristic_gene_ranking.py`
//! fails by name until `tools/heuristic_gene_ranking.py --write` has run. The
//! regenerated file then has to be added to the PR's `Claimed paths:` line, or
//! `collaboration-policy` refuses it as unclaimed.

use super::*;
use crate::ai::Ai;
use crate::game::{Action, Game};
use crate::setup::GameSpeed;

/// A named census arm and the genes it turns on.
type Arm = (&'static str, fn(&mut AdvancedAi));

/// One seat's research chain at the end of a game.
struct Chain {
    cities: usize,
    campus: usize,
    library: usize,
    university: usize,
    research_lab: usize,
    techs: usize,
    science: f64,
    score: i64,
}

fn play(seed: u64, arm: impl Fn(&mut AdvancedAi)) -> Chain {
    // The screen's own profile, so a reading here is comparable with one
    // there: six players, 60x38, Online, 250 turns.
    let mut g = Game::new(6, 60, 38, seed, 250, 6);
    g.game_speed = GameSpeed::Online;
    // ⚠ THE SAME CONDITIONING THE SCREEN NEEDED, FOR THE SAME REASON. Left at
    // the default six lanes, a native six-player game ends by RELIGIOUS
    // conversion 76% of the time at a median turn 147 of 250 — so the late
    // game these genes exist to price does not happen in three games out of
    // four, and a census of where the research chain stops would be a census
    // of where it stops at turn 147. The first run of this census returned 32
    // technologies a game against the screen's 62 for exactly that reason.
    g.victory_conditions =
        crate::game::VictoryConditions::parse("science,culture,domination,score")
            .expect("the screen's own lanes");
    let mut me = AdvancedAi::new();
    // `gene_screen::treated_seat` starts here, then sets each gene.
    me.enable_engine_repairs_universe();
    arm(&mut me);
    let mut others = AdvancedAi::fleet(&g);
    while g.winner.is_none() && g.turn <= g.max_turns {
        let pid = g.current;
        if pid == 0 {
            me.take_turn(&mut g, pid);
        } else {
            others[pid].take_turn(&mut g, pid);
        }
        if g.winner.is_none() && g.current == pid {
            let _ = g.apply(pid, &Action::EndTurn);
        }
    }
    let cities = g.player_city_ids(0);
    let holding = |building: &str| {
        cities
            .iter()
            .filter(|c| g.cities[c].buildings.contains(&Name::new(building)))
            .count()
    };
    Chain {
        cities: cities.len(),
        campus: cities
            .iter()
            .filter(|c| {
                g.cities[c]
                    .districts
                    .keys()
                    .any(|d| g.district_family(*d) == crate::name!("campus"))
            })
            .count(),
        library: holding("library"),
        university: holding("university"),
        research_lab: holding("research_lab"),
        techs: g.players[0].techs.len(),
        science: cities.iter().map(|c| g.city_yields(*c).science).sum(),
        score: g.score(0),
    }
}

fn report(label: &str, arms: &[(&str, Chain)]) {
    println!("\n== {label} ==");
    println!(
        "{:<28}{:>7}{:>8}{:>9}{:>7}{:>7}{:>8}{:>9}{:>8}",
        "arm", "cities", "campus", "library", "univ", "lab", "techs", "science", "score"
    );
    for (name, c) in arms {
        println!(
            "{:<28}{:>7}{:>8}{:>9}{:>7}{:>7}{:>8}{:>9.1}{:>8}",
            name,
            c.cities,
            c.campus,
            c.library,
            c.university,
            c.research_lab,
            c.techs,
            c.science,
            c.score
        );
    }
}

#[test]
#[ignore = "census, not an assertion; run explicitly with --nocapture"]
fn the_research_chain_treated_against_control() {
    let seeds: Vec<u64> = (0..12).map(|i| 86_000_000 + i).collect();
    // ⚠ TRIMMED TO THE QUESTION, ROUND THREE. Twelve seeds settled the two
    // building-half genes: `science-multiplier-payoff` +25% Research Labs and
    // +11.9% Science, `research-tier-premium` +20% and +13.5%, both together
    // +30% and +13.1%. What is unmeasured is `power-the-laboratory`, which
    // aims at the same rung from the other side — the Lab's `powered_science`
    // 5 is switched off until something generates power, and nothing in the
    // controller bought the switch. So: does it help alone, and does it add to
    // the stack that already works?
    let arms: Vec<Arm> = vec![
        ("control (universe)", |_ai| {}),
        ("power-the-laboratory", |ai| {
            ai.enable_power_the_laboratory()
        }),
        ("premium + payoff", |ai| {
            ai.enable_research_tier_premium();
            ai.enable_science_multiplier_payoff();
        }),
        ("premium + payoff + power", |ai| {
            ai.enable_research_tier_premium();
            ai.enable_science_multiplier_payoff();
            ai.enable_power_the_laboratory();
        }),
    ];
    let mut totals: Vec<(String, Chain)> = Vec::new();
    for (name, arm) in &arms {
        let mut sum = Chain {
            cities: 0,
            campus: 0,
            library: 0,
            university: 0,
            research_lab: 0,
            techs: 0,
            science: 0.0,
            score: 0,
        };
        for seed in &seeds {
            let c = play(*seed, arm);
            sum.cities += c.cities;
            sum.campus += c.campus;
            sum.library += c.library;
            sum.university += c.university;
            sum.research_lab += c.research_lab;
            sum.techs += c.techs;
            sum.science += c.science;
            sum.score += c.score;
        }
        println!(
            "{name}: cities {} campus {} library {} univ {} lab {} techs {} sci {:.1} score {}",
            sum.cities,
            sum.campus,
            sum.library,
            sum.university,
            sum.research_lab,
            sum.techs,
            sum.science,
            sum.score
        );
        totals.push((name.to_string(), sum));
    }
    report(
        &format!("summed over {} seeds", seeds.len()),
        &totals
            .iter()
            .map(|(n, c)| (n.as_str(), Chain { ..*c }))
            .collect::<Vec<_>>(),
    );
}
