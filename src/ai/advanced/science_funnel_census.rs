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
    let seeds: Vec<u64> = (0..12).map(|i| 87_000_000 + i).collect();
    // ⚠ ROUND FOUR: the gate, not the price. The two building-half genes are
    // settled (+25%/+20% Labs, +12–13.5% Science) and `power-the-laboratory`
    // is a measured null. What is open is the multiplier half NO Campus in the
    // empire clears — 0 of 9, with four cities holding a free adjacency-4 plot.
    let arms: Vec<Arm> = vec![
        ("control (universe)", |_ai| {}),
        ("campus-adjacency-threshold", |ai| {
            ai.enable_campus_adjacency_threshold()
        }),
        ("premium + payoff", |ai| {
            ai.enable_research_tier_premium();
            ai.enable_science_multiplier_payoff();
        }),
        ("premium + payoff + threshold", |ai| {
            ai.enable_research_tier_premium();
            ai.enable_science_multiplier_payoff();
            ai.enable_campus_adjacency_threshold();
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

#[cfg(test)]
mod power_probe {
    use super::*;
    /// Does the decision a gene prices ever actually ARISE?
    ///
    /// ★★★★★ THE CHECK THAT WOULD HAVE SAVED TWO GENES. `campus-finishes-first`
    /// and `power-the-laboratory` both came back byte-identical to control —
    /// the second over TWELVE seeds — and in both cases the code was correct,
    /// wired, and reachable. What was missing was a board where the choice it
    /// changes is ever offered.
    ///
    /// For the power gene this probe answers it in one game: **10 cities, 4
    /// with an Industrial Zone, 4 drawing power, 3 ALREADY HOLDING A PLANT,
    /// and exactly ONE left dark.** The empire builds its power plants for
    /// reasons that have nothing to do with beakers — a Factory and a plant
    /// are production — so the Research Lab's `powered_science` 5 is switched
    /// on nearly everywhere already, and a gene that pays for the switch has
    /// almost nothing left to buy. The premise was right and the opportunity
    /// is not there.
    ///
    /// Run this BEFORE spending census games on a gene, not after.
    #[test]
    #[ignore = "probe"]
    fn does_the_empire_ever_face_the_power_decision() {
        let mut g = Game::new(6, 60, 38, 86_000_000, 250, 6);
        g.game_speed = GameSpeed::Online;
        g.victory_conditions =
            crate::game::VictoryConditions::parse("science,culture,domination,score").unwrap();
        let mut me = AdvancedAi::new();
        me.enable_engine_repairs_universe();
        me.enable_power_the_laboratory();
        let mut others = AdvancedAi::fleet(&g);
        while g.winner.is_none() && g.turn <= 250 {
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
        let plants = ["coal_power_plant", "oil_power_plant", "nuclear_power_plant"];
        let mut industrial = 0;
        let mut dark = 0;
        let mut demanding = 0;
        let mut holds_plant = 0;
        let mut plant_producible = 0;
        for cid in &cities {
            let city = &g.cities[cid];
            if g.city_has_district_family(city, crate::name!("industrial_zone")) {
                industrial += 1;
            }
            if g.city_power_demand(city) > 0.0 {
                demanding += 1;
            }
            if !g.city_is_powered(city) {
                dark += 1;
            }
            for plant in plants {
                if city.buildings.contains(&Name::new(plant)) {
                    holds_plant += 1;
                }
                if g.can_produce(
                    0,
                    *cid,
                    &crate::game::Item::Building {
                        building: Name::new(plant),
                    },
                ) {
                    plant_producible += 1;
                }
            }
        }
        println!(
            "POWER cities={} industrial_zone={industrial} demanding_power={demanding} \
             dark={dark} holds_plant={holds_plant} plant_producible_now={plant_producible} \
             techs={}",
            cities.len(),
            g.players[0].techs.len()
        );
    }
}

#[cfg(test)]
mod science_gates_probe {
    use super::*;

    /// Where the science multipliers are LOST, counted at the end of a game.
    ///
    /// The two genes that work both price a Campus building better. What is
    /// left is the multipliers on top of them, and every one is gated on
    /// something the empire may or may not have reached. This counts the
    /// gates rather than guessing at them — see the power probe above for why
    /// that order matters.
    #[test]
    #[ignore = "probe"]
    fn which_science_multiplier_gates_does_the_empire_actually_clear() {
        let mut g = Game::new(6, 60, 38, 86_000_100, 250, 6);
        g.game_speed = GameSpeed::Online;
        g.victory_conditions =
            crate::game::VictoryConditions::parse("science,culture,domination,score").unwrap();
        let mut me = AdvancedAi::new();
        me.enable_engine_repairs_universe();
        me.enable_research_tier_premium();
        me.enable_science_multiplier_payoff();
        // Toggle to read the gate with and without the gene that prices it.
        if std::env::var("CIVVIS_PROBE_THRESHOLD").is_ok() {
            me.enable_campus_adjacency_threshold();
        }
        let campus = crate::name!("campus");
        // ⚠⚠ THE SITE SURVEY HAS TO BE TAKEN WHEN THE CAMPUS IS SITED, NOT AT
        // THE END. District adjacency counts NEIGHBOURING DISTRICTS, so a plot
        // that reads 4.0 at turn 250 may have been worth 1 when the Campus was
        // placed — the empire's own later districts made it good. Reading the
        // end state and calling it an available choice is the same mistake as
        // matching a per-turn snapshot to an event by turn number.
        // ⚠⚠ SURVEY THE LEGAL SITES, NOT EVERY OWNED TILE. A first draft scored
        // adjacency on every tile the city owned and reported a 5.0 "available"
        // — but a Campus cannot stand on a mountain, and a tile RINGED by
        // mountains is exactly where the adjacency arithmetic peaks. The tiles
        // with the best numbers were the ones no Campus can ever occupy.
        // `district_sites` is the engine's own answer to "where could this go",
        // and it is the only honest denominator.
        let survey = |g: &Game, cid: u32| -> f64 {
            let mut best = 0.0_f64;
            for position in g.district_sites(cid, campus) {
                let mut yields = Yields::default();
                for source in g.district_adjacency_sources(campus, position) {
                    if source.source != "adjacency_bonus" {
                        yields.add(source.yields);
                    }
                }
                best = best.max(yields.science);
            }
            best
        };
        let mut best_at_siting: Vec<f64> = Vec::new();
        let mut sited: std::collections::BTreeSet<u32> = Default::default();
        let mut carried: std::collections::BTreeMap<u32, f64> = Default::default();
        let mut offers: std::collections::BTreeMap<u32, Vec<f64>> = Default::default();
        let mut offered_at_siting: Vec<Vec<f64>> = Vec::new();
        let mut others = AdvancedAi::fleet(&g);
        while g.winner.is_none() && g.turn <= 250 {
            let pid = g.current;
            if pid == 0 {
                // ⚠ A district completes during END-TURN processing, not
                // inside `take_turn`. A first draft checked immediately after
                // the seat's turn and recorded NOTHING — an empty list, which
                // is the tell. Carry each city's survey forward instead and
                // read it back on the turn the Campus actually appears.
                for cid in g.player_city_ids(0) {
                    if sited.contains(&cid) {
                        continue;
                    }
                    if g.city_has_district_family(&g.cities[&cid], campus) {
                        sited.insert(cid);
                        best_at_siting.push(carried.get(&cid).copied().unwrap_or(-1.0));
                        offered_at_siting.push(offers.get(&cid).cloned().unwrap_or_default());
                    } else {
                        carried.insert(cid, survey(&g, cid));
                        // ⭐ AND WHAT WAS ACTUALLY ON THE TABLE. `producible_items`
                        // offers at most the best TWO fresh sites per district,
                        // ranked by `district_yields(...).total()`. If the good
                        // plot is not in that pair, no pricing term downstream
                        // can ever choose it — which is the difference between
                        // "the chooser declined it" and "the chooser never saw
                        // it", and they need opposite fixes.
                        let offered: Vec<f64> = g
                            .producible_items(0, cid)
                            .into_iter()
                            .filter_map(|item| match item {
                                crate::game::Item::District { district, pos }
                                    if g.district_family(district) == campus =>
                                {
                                    let mut yields = Yields::default();
                                    for source in g.district_adjacency_sources(campus, pos) {
                                        if source.source != "adjacency_bonus" {
                                            yields.add(source.yields);
                                        }
                                    }
                                    Some((yields.science * 10.0).round() / 10.0)
                                }
                                _ => None,
                            })
                            .collect();
                        if !offered.is_empty() {
                            offers.insert(cid, offered);
                        }
                    }
                }
                me.take_turn(&mut g, pid);
            } else {
                others[pid].take_turn(&mut g, pid);
            }
            if g.winner.is_none() && g.current == pid {
                let _ = g.apply(pid, &Action::EndTurn);
            }
        }
        println!(
            "OFFERED Campus plot adjacencies producible_items actually put on the \
             table, per Campus: {offered_at_siting:?}"
        );
        best_at_siting.sort_by(|a, b| b.partial_cmp(a).unwrap());
        println!(
            "AT-SITING best free Campus adjacency the chooser actually had, per Campus: {:?} \
             (>=4 available on {} of {})",
            best_at_siting
                .iter()
                .map(|v| (v * 10.0).round() / 10.0)
                .collect::<Vec<_>>(),
            best_at_siting.iter().filter(|v| **v >= 4.0).count(),
            best_at_siting.len()
        );
        let cities = g.player_city_ids(0);
        let (mut with_campus, mut pop15, mut adj4, mut both) = (0, 0, 0, 0);
        let mut pops: Vec<i32> = Vec::new();
        for cid in &cities {
            let city = &g.cities[cid];
            pops.push(city.pop);
            if !g.city_has_district_family(city, campus) {
                continue;
            }
            with_campus += 1;
            // The two halves `Game::city_yields` gates the Campus card on.
            let big = city.pop >= 15;
            let adjacency = g
                .city_district_family_position(city, campus)
                .map(|position| {
                    let placed = g.map.tiles[&position].district.unwrap_or(campus);
                    let mut yields = Yields::default();
                    for source in g.district_adjacency_sources(placed, position) {
                        if source.source != "adjacency_bonus" {
                            yields.add(source.yields);
                        }
                    }
                    yields.science
                })
                .unwrap_or(0.0);
            let sharp = adjacency >= 4.0;
            if big {
                pop15 += 1;
            }
            if sharp {
                adj4 += 1;
            }
            if big && sharp {
                both += 1;
            }
        }
        // ⭐ AND THE CHECK THE POWER PROBE TAUGHT: does the choice even exist?
        // A Campus that cannot reach adjacency 4 anywhere in its own work
        // radius is not a siting mistake, it is a map. Count the best Campus
        // plot each city actually had.
        let mut could_reach_four = 0;
        let mut best_available: Vec<f64> = Vec::new();
        for cid in &cities {
            // Same legal denominator as the siting survey above.
            let best = survey(&g, *cid);
            best_available.push(best);
            if best >= 4.0 {
                could_reach_four += 1;
            }
        }
        best_available.sort_by(|a, b| b.partial_cmp(a).unwrap());
        println!(
            "SITES cities_with_a_reachable_adj4_plot={could_reach_four}              best_free_campus_adjacency_per_city={:?}",
            best_available
                .iter()
                .map(|v| (v * 10.0).round() / 10.0)
                .collect::<Vec<_>>()
        );

        pops.sort_unstable();
        let median = pops.get(pops.len() / 2).copied().unwrap_or(0);
        // ⭐ THE OTHER HALF, AND THE ONLY REACHABLE ONE. The adjacency gate is
        // dead on this profile — under 1% of land can host a Campus that
        // clears it (see `how_much_of_the_map_could_ever_host_a_high_adjacency
        // _campus`). The POPULATION gate is a different story: the median
        // Campus city sits at 12 against a threshold of 15. Whether that is a
        // choice or a ceiling depends on whether those cities can still grow,
        // so count the headroom rather than assuming it.
        let mut short_of_fifteen = 0;
        let mut short_but_growing = 0;
        let mut short_and_capped = 0;
        let mut pop_gap_total = 0;
        for cid in &cities {
            let city = &g.cities[cid];
            if !g.city_has_district_family(city, campus) || city.pop >= 15 {
                continue;
            }
            short_of_fifteen += 1;
            pop_gap_total += 15 - city.pop;
            let yields = g.city_yields(*cid);
            let housing = g.city_housing(city);
            if housing > city.pop as f64 && yields.food > 0.0 {
                short_but_growing += 1;
            } else {
                short_and_capped += 1;
            }
        }
        println!(
            "POPGATE campus_cities_under_15={short_of_fifteen} \
             still_growing={short_but_growing} capped={short_and_capped} \
             total_pop_short={pop_gap_total}"
        );

        let rationalism = g.players[0].policies.contains(&crate::name!("rationalism"));
        let philosophy = g.players[0]
            .policies
            .contains(&crate::name!("natural_philosophy"));
        let suzerainties = g
            .players
            .iter()
            .filter(|p| p.is_minor && p.alive)
            .filter(|p| g.suzerain_of(p.id) == Some(0))
            .count();
        println!(
            "GATES cities={} campus={with_campus} pop>=15:{pop15} adj>=4:{adj4} both:{both} \
             median_pop={median} max_pop={} rationalism={rationalism} \
             natural_philosophy={philosophy} suzerainties={suzerainties} techs={} sci={:.1}",
            cities.len(),
            pops.last().copied().unwrap_or(0),
            g.players[0].techs.len(),
            cities
                .iter()
                .map(|c| g.city_yields(*c).science)
                .sum::<f64>()
        );
    }
}

#[cfg(test)]
mod map_reach_probe {
    use super::*;

    /// Is the Campus multiplier's adjacency gate reachable ON THIS MAP AT ALL?
    ///
    /// The third survey correction established that no LEGAL Campus plot in
    /// the empire ever reached 4 raw Science. That leaves two very different
    /// worlds: the seat founds its cities in the wrong places, or the profile
    /// the screen runs simply has nowhere to put such a Campus. Only the first
    /// is worth a gene, and this counts which it is — over the whole map,
    /// before anyone has built anything.
    #[test]
    #[ignore = "probe"]
    fn how_much_of_the_map_could_ever_host_a_high_adjacency_campus() {
        let campus = crate::name!("campus");
        let mut totals = Vec::new();
        for seed in 0..3u64 {
            let g = Game::new(6, 60, 38, 87_000_000 + seed, 250, 6);
            let mut buckets = [0usize; 6];
            let mut land = 0usize;
            for (position, tile) in g.map.tiles.iter() {
                // Only ground a Campus could stand on; the mountains that make
                // the arithmetic look good are exactly where it cannot go.
                if tile.terrain == crate::name!("ocean")
                    || tile.terrain == crate::name!("coast")
                    || tile.terrain == crate::name!("mountain")
                    || tile.terrain == crate::name!("lake")
                {
                    continue;
                }
                land += 1;
                let mut yields = Yields::default();
                for source in g.district_adjacency_sources(campus, *position) {
                    if source.source != "adjacency_bonus" {
                        yields.add(source.yields);
                    }
                }
                let bucket = (yields.science.max(0.0) as usize).min(5);
                buckets[bucket] += 1;
            }
            totals.push((seed, land, buckets));
        }
        for (seed, land, buckets) in &totals {
            let four_plus = buckets[4] + buckets[5];
            println!(
                "MAP seed {} land_plots={land} adjacency 0:{} 1:{} 2:{} 3:{} 4+:{} \
                 ({:.2}% of land could host a gate-clearing Campus)",
                87_000_000 + seed,
                buckets[0],
                buckets[1],
                buckets[2],
                buckets[3],
                four_plus,
                100.0 * four_plus as f64 / (*land).max(1) as f64
            );
        }
    }
}
