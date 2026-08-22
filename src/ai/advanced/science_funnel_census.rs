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
//! ★★ AND THE ANSWER DEPENDS ON THE CONTROL'S HEADROOM. `science-multiplier-
//! payoff` + `research-tier-premium` bought Research Labs **+25%, +20%,
//! +21.7%, +6.7% and finally 0%** across five disjoint seed ranges. The trend
//! is not noise: the last range's CONTROL already reached 115 Campuses and 65
//! Labs, so there was nothing left to add. A gene that fills a gap is worth
//! what the gap is, and quoting its best range as its effect overstates it.
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
    let seeds: Vec<u64> = (0..12).map(|i| 92_000_000 + i).collect();
    // ⚠ ROUND NINE: the first empire-wide science multiplier in this campaign.
    // Every lever priced so far was per-city. `great-person-effect-reach` fixes
    // a patronage ranking that sums a per-building RATE with a one-off LUMP —
    // Einstein's `research_labs_science: 4` scores five against Wernher von
    // Braun's fourteen hundred — and the probe confirms the seat really does
    // recruit one to three Great Scientists a game, so the decision arises.
    let arms: Vec<Arm> = vec![
        ("control (universe)", |_ai| {}),
        ("great-person-effect-reach", |ai| {
            ai.enable_great_person_effect_reach()
        }),
        ("premium + payoff", |ai| {
            ai.enable_research_tier_premium();
            ai.enable_science_multiplier_payoff();
        }),
        ("premium + payoff + reach", |ai| {
            ai.enable_research_tier_premium();
            ai.enable_science_multiplier_payoff();
            ai.enable_great_person_effect_reach();
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

#[cfg(test)]
mod envoy_and_deck_probe {
    use super::*;

    /// The two science multipliers the gate census left unexplained: a
    /// suzerainty count of ZERO, and Natural Philosophy never slotted.
    ///
    /// `international_space_agency` is `science_pct_per_suzerain: 5`, so four
    /// suzerainties would be +20% empire science — and the seat ends with
    /// none. `natural_philosophy` doubles a Campus's adjacency yield (it
    /// cannot open the Population/adjacency GATE, which `city_yields` reads
    /// raw, but it doubles what the district earns), and
    /// `strategic_policies` inserts it beside Rationalism the moment the
    /// empire owns a Campus.
    ///
    /// ★★★★ ANSWERED, AND THE ANSWER IS THAT NEITHER IS A GENE. Measured on
    /// the screen's own profile:
    ///
    /// ```text
    /// DECK  running = cryptography, five_year_plan, gunboat_diplomacy,
    ///                 levee_en_masse, new_deal, rationalism, wisselbanken
    ///       slots   = military 1 · ECONOMIC 2 · diplomatic 3 · wildcard 1
    /// ENVOYS unspent 0 · peak unspent 1 · PLACED 16
    /// city-states alive 4 · met 4 · ours 0 (PEAK 2) · taken by rivals 2
    /// ```
    ///
    /// **Natural Philosophy is not missing, it is homeless.** There are TWO
    /// economic slots and Rationalism is in one of them; the card that would
    /// double a Campus's adjacency has nowhere to go, and Rationalism is the
    /// better of the two anyway. No pricing term creates a slot.
    ///
    /// **The envoys are not banked either** — 16 placed, 0 unspent, so the
    /// native seat does not have the live seat's 56-envoy hole. What it has is
    /// a LOSS: suzerainty peaked at 2 and ended at 0, with 2 of the four
    /// living city-states held by rivals. That is an influence race, not a
    /// valuation gap, and `international_space_agency` would need a slot the
    /// deck has not got either.
    ///
    /// ⇒ The science-multiplier lane is exhausted for pricing genes. What is
    /// left of it is slot scarcity (a government question) and an influence
    /// race (a diplomatic one). The lever that works stays the one already
    /// merged: buy more Campus BUILDINGS.
    #[test]
    #[ignore = "probe"]
    fn where_do_the_envoys_and_the_science_cards_go() {
        let mut g = Game::new(6, 60, 38, 88_000_000, 250, 6);
        g.game_speed = GameSpeed::Online;
        g.victory_conditions =
            crate::game::VictoryConditions::parse("science,culture,domination,score").unwrap();
        let mut me = AdvancedAi::new();
        me.enable_engine_repairs_universe();
        let mut others = AdvancedAi::fleet(&g);
        let mut peak_envoys = 0i64;
        let mut peak_suzerain = 0usize;
        while g.winner.is_none() && g.turn <= 250 {
            let pid = g.current;
            if pid == 0 {
                me.take_turn(&mut g, pid);
                peak_envoys = peak_envoys.max(g.players[0].envoys_free);
                peak_suzerain = peak_suzerain.max(
                    g.players
                        .iter()
                        .filter(|p| p.is_minor && p.alive)
                        .filter(|p| g.suzerain_of(p.id) == Some(0))
                        .count(),
                );
            } else {
                others[pid].take_turn(&mut g, pid);
            }
            if g.winner.is_none() && g.current == pid {
                let _ = g.apply(pid, &Action::EndTurn);
            }
        }
        let minors: Vec<usize> = g
            .players
            .iter()
            .filter(|p| p.is_minor && p.alive)
            .map(|p| p.id)
            .collect();
        let met = minors.iter().filter(|m| g.has_met(0, **m)).count();
        let ours = minors
            .iter()
            .filter(|m| g.suzerain_of(**m) == Some(0))
            .count();
        let taken = minors
            .iter()
            .filter(|m| g.suzerain_of(**m).is_some_and(|s| s != 0))
            .count();
        // The deck: what the seat is actually running at the end.
        let slotted: Vec<&str> = [
            "rationalism",
            "natural_philosophy",
            "international_space_agency",
        ]
        .into_iter()
        .filter(|card| g.players[0].policies.contains(&Name::new(card)))
        .collect();
        // What is actually running, and whether anything could have joined it.
        let running: Vec<String> = g.players[0]
            .policies
            .iter()
            .map(|card| card.to_string())
            .collect();
        let slots = g.gov_slots(0);
        println!("DECK running={running:?} slots={slots:?}");
        println!(
            "ENVOYS unspent_at_end={} peak_unspent={peak_envoys} placed={} · \
             city_states alive={} met={met} OURS={ours} (peak {peak_suzerain}) \
             taken_by_rivals={taken} · science_cards_slotted={slotted:?} · \
             cards_running={}",
            g.players[0].envoys_free,
            g.players[0].envoys.iter().map(|(_, n)| n).sum::<i64>(),
            minors.len(),
            g.players[0].policies.len()
        );
    }
}

#[cfg(test)]
mod chain_tech_probe {
    use super::*;

    /// When does the chain's technology actually arrive, and how many turns
    /// does each rung then have to be built in?
    ///
    /// The census leaves 40 of 100 Campuses without a Research Lab even with
    /// the two genes that work. The Lab costs **440** and is gated on
    /// `chemistry`; the University costs 250 and is gated on `education`. If
    /// those nodes land late, the gap is a CLOCK problem and no pricing term
    /// closes it — which is exactly the distinction that killed four genes in
    /// this bundle.
    ///
    /// ★★★★ MEASURED, AND IT IS THE CLOCK. Three seeds, both merged genes on:
    ///
    /// ```text
    /// seed  writing  library    education  university  CHEMISTRY  lab      labs
    /// 0     t58      stands t95 t88        stands t111 t147       t158       8
    /// 1     t53      stands t69 t78        stands t85  t134       t151       8
    /// 2     t30      stands t120 t119      stands t132 t205       never      0
    /// ```
    ///
    /// Where Chemistry lands by ~t147 the Lab gets built in every Campus that
    /// has a University; where it lands at **t205 there are none at all.** The
    /// pricing is not the binding constraint at that point — the tech is.
    ///
    /// And the delay compounds because `unreachable_science_building_tech`
    /// only aims at a rung whose prerequisite buildings ALREADY STAND, so
    /// every build time is added to every later tech's start: Chemistry
    /// follows the University STANDING by 36, 49 and 73 turns.
    ///
    /// ⚠ Seed 2 also shows the other half of it, and the merged genes do not
    /// fix it: **Writing at t30 and the first Library standing at t120** — a
    /// Campus without its Library for ninety turns.
    #[test]
    #[ignore = "probe"]
    fn when_does_the_research_chain_become_buildable() {
        for seed in 0..3u64 {
            let mut g = Game::new(6, 60, 38, 88_000_000 + seed, 250, 6);
            g.game_speed = GameSpeed::Online;
            g.victory_conditions =
                crate::game::VictoryConditions::parse("science,culture,domination,score").unwrap();
            let mut me = AdvancedAi::new();
            me.enable_engine_repairs_universe();
            me.enable_research_tier_premium();
            me.enable_science_multiplier_payoff();
            let mut others = AdvancedAi::fleet(&g);
            let mut arrived: std::collections::BTreeMap<&str, u32> = Default::default();
            let mut built: std::collections::BTreeMap<&str, u32> = Default::default();
            while g.winner.is_none() && g.turn <= 250 {
                let pid = g.current;
                if pid == 0 {
                    me.take_turn(&mut g, pid);
                    for tech in ["writing", "education", "chemistry"] {
                        if !arrived.contains_key(tech)
                            && g.players[0].techs.contains(&Name::new(tech))
                        {
                            arrived.insert(tech, g.turn);
                        }
                    }
                    // ⭐ AND WHEN EACH RUNG STANDS, because that is what gates
                    // the NEXT tech goal: `unreachable_science_building_tech`
                    // only aims at a rung whose prerequisite buildings ALREADY
                    // STAND, so every build time is added to every later
                    // tech's start.
                    for rung in ["library", "university", "research_lab"] {
                        if !built.contains_key(rung)
                            && g.player_city_ids(0)
                                .into_iter()
                                .any(|cid| g.cities[&cid].buildings.contains(&Name::new(rung)))
                        {
                            built.insert(rung, g.turn);
                        }
                    }
                } else {
                    others[pid].take_turn(&mut g, pid);
                }
                if g.winner.is_none() && g.current == pid {
                    let _ = g.apply(pid, &Action::EndTurn);
                }
            }
            let cities = g.player_city_ids(0);
            let holding = |name: &str| {
                cities
                    .iter()
                    .filter(|c| g.cities[c].buildings.contains(&Name::new(name)))
                    .count()
            };
            println!(
                "CHAINTECH seed {} tech writing=t{:?} education=t{:?} CHEMISTRY=t{:?} \
                 | first stood library=t{:?} university=t{:?} lab=t{:?} \
                 | cities={} library={} university={} lab={}",
                88_000_000 + seed,
                arrived.get("writing"),
                arrived.get("education"),
                arrived.get("chemistry"),
                built.get("library"),
                built.get("university"),
                built.get("research_lab"),
                cities.len(),
                holding("library"),
                holding("university"),
                holding("research_lab"),
            );
        }
    }
}

#[cfg(test)]
mod variance_probe {
    use super::*;

    /// What separates a good science game from a bad one?
    ///
    /// ★★ THE CONTROL'S OWN SPREAD IS LARGER THAN ANY GENE IN THIS BUNDLE.
    /// Across four census rounds the untreated seat finished with **40, 46, 60
    /// and 65 Research Labs** and **2370, 2591, 3512 and 3483 Science** — and
    /// the two genes that help move Labs by a fifth *where there is a gap*.
    /// Explaining the spread is worth more than another marginal price.
    ///
    /// One game in the seed-88 round is the extreme: Writing at t30 and the
    /// first Library standing at **t120**, nine cities, four Libraries, ZERO
    /// Research Labs. Another on the same profile had twelve cities and eight
    /// Labs. Whatever separates those two is the real science lever.
    ///
    /// This reports one line per seed so the driver can be read off rather
    /// than guessed at: when the first Campus stood, when the first Library
    /// stood, and what the empire finished with.
    ///
    /// ★★★★★ ANSWERED, AND IT REDIRECTS THE WHOLE LANE. Twelve seeds, the
    /// untreated seat:
    ///
    /// ```text
    ///     seed campus1 library1  chem  cities campus lib lab  science  score
    /// 89000008      54       63   137      17     16  16   8    460.1   1384
    /// 89000003      78       86   146      13     12  11   8    413.8   1032
    /// 89000001      50       54   148      16     14  14   8    350.7   1058
    /// 89000000      84       84   141       9      8   8   6    356.2    894
    /// 89000010      58       65   123      10      8   8   8    296.6   1066
    /// 89000009     145      149   211       9      7   7   2    145.5    548
    /// 89000002      53      125  never      9      6   6   0    100.5    495
    /// ```
    ///
    ///     corr(first Campus turn, Science)  = -0.30
    ///     corr(first Library turn, Science) = -0.64
    ///     corr(cities,             Science) = +0.68
    ///
    /// **Cities predict Science better than anything this bundle prices**, and
    /// the first Library's turn is nearly as strong the other way. The two
    /// worst games are the two late ones: seed 9 did not stand a Campus until
    /// **t145**, and seed 2 never researched Chemistry at all — nought Labs,
    /// a hundred Science, half the score of the median game.
    ///
    /// ⇒ **Late-game science in this engine is downstream of expansion and of
    /// the chain's EARLY timing.** It is not a late-game quantity at all. That
    /// is why nine genes aimed at the late game produced two conditional wins
    /// and five nulls, and why the one that tried to force the tech order was
    /// the worst of them — it bought Chemistry with the expansion Science is
    /// made of (`chain_tech_lookahead`: -17% Labs and SEVENTEEN fewer cities).
    ///
    /// The operator's premise is right — the same table shows Science and
    /// score moving together, 460/1384 at the top and 100/495 at the bottom.
    /// The lever is just not where the pricing lives: it is `land_grab`,
    /// `wide-map-capacity` and whatever gets the first Campus standing before
    /// turn 60.
    #[test]
    #[ignore = "census, not an assertion; run explicitly with --nocapture"]
    fn what_separates_a_good_science_game_from_a_bad_one() {
        let campus = crate::name!("campus");
        println!(
            "{:>10}{:>8}{:>9}{:>9}{:>8}{:>8}{:>7}{:>7}{:>10}{:>8}",
            "seed",
            "campus1",
            "library1",
            "chem",
            "cities",
            "campus",
            "lib",
            "lab",
            "science",
            "score"
        );
        let mut rows = Vec::new();
        for seed in 0..12u64 {
            let mut g = Game::new(6, 60, 38, 89_000_000 + seed, 250, 6);
            g.game_speed = GameSpeed::Online;
            g.victory_conditions =
                crate::game::VictoryConditions::parse("science,culture,domination,score").unwrap();
            let mut me = AdvancedAi::new();
            me.enable_engine_repairs_universe();
            let mut others = AdvancedAi::fleet(&g);
            let (mut campus1, mut library1, mut chem) = (0u32, 0u32, 0u32);
            while g.winner.is_none() && g.turn <= 250 {
                let pid = g.current;
                if pid == 0 {
                    me.take_turn(&mut g, pid);
                    let cities = g.player_city_ids(0);
                    if campus1 == 0
                        && cities
                            .iter()
                            .any(|c| g.city_has_district_family(&g.cities[c], campus))
                    {
                        campus1 = g.turn;
                    }
                    if library1 == 0
                        && cities
                            .iter()
                            .any(|c| g.cities[c].buildings.contains(&crate::name!("library")))
                    {
                        library1 = g.turn;
                    }
                    if chem == 0 && g.players[0].techs.contains(&crate::name!("chemistry")) {
                        chem = g.turn;
                    }
                } else {
                    others[pid].take_turn(&mut g, pid);
                }
                if g.winner.is_none() && g.current == pid {
                    let _ = g.apply(pid, &Action::EndTurn);
                }
            }
            let cities = g.player_city_ids(0);
            let holding = |name: &str| {
                cities
                    .iter()
                    .filter(|c| g.cities[c].buildings.contains(&Name::new(name)))
                    .count()
            };
            let science: f64 = cities.iter().map(|c| g.city_yields(*c).science).sum();
            let campuses = cities
                .iter()
                .filter(|c| g.city_has_district_family(&g.cities[c], campus))
                .count();
            let labs = holding("research_lab");
            println!(
                "{:>10}{:>8}{:>9}{:>9}{:>8}{:>8}{:>7}{:>7}{:>10.1}{:>8}",
                89_000_000 + seed,
                campus1,
                library1,
                chem,
                cities.len(),
                campuses,
                holding("library"),
                labs,
                science,
                g.score(0)
            );
            rows.push((
                campus1 as f64,
                library1 as f64,
                cities.len() as f64,
                science,
            ));
        }
        // The correlation the table is for, stated rather than eyeballed.
        let corr = |pick: fn(&(f64, f64, f64, f64)) -> f64| {
            let n = rows.len() as f64;
            let (mx, my) = (
                rows.iter().map(pick).sum::<f64>() / n,
                rows.iter().map(|r| r.3).sum::<f64>() / n,
            );
            let cov: f64 = rows.iter().map(|r| (pick(r) - mx) * (r.3 - my)).sum();
            let vx: f64 = rows.iter().map(|r| (pick(r) - mx).powi(2)).sum();
            let vy: f64 = rows.iter().map(|r| (r.3 - my).powi(2)).sum();
            if vx <= 0.0 || vy <= 0.0 {
                0.0
            } else {
                cov / (vx * vy).sqrt()
            }
        };
        println!(
            "VARIANCE corr(first Campus turn, Science) = {:+.2} · \
             corr(first Library turn, Science) = {:+.2} · \
             corr(cities, Science) = {:+.2}",
            corr(|r| r.0),
            corr(|r| r.1),
            corr(|r| r.2)
        );
    }
}

#[cfg(test)]
mod finished_city_probe {
    use super::*;

    /// What does a city with a FINISHED research chain build for the rest of
    /// the game?
    ///
    /// The variance probe showed Science is downstream of expansion and early
    /// timing — but it did not ask what the cities that DID finish their chain
    /// do afterwards. From about turn 150 a Campus city holding its Library,
    /// University and Research Lab has nothing left in the chain to build, and
    /// whatever it picks instead is the empire's remaining late-game science
    /// decision. If those turns go to repeatable projects with no beaker in
    /// them, or to units, that is a late-game science lever that no pricing on
    /// the CHAIN can reach.
    ///
    /// Counts what finished Campus cities actually queue over the last
    /// hundred turns, by item family.
    #[test]
    #[ignore = "census, not an assertion; run explicitly with --nocapture"]
    fn what_does_a_finished_research_city_do_with_its_last_hundred_turns() {
        let campus = crate::name!("campus");
        let chain = ["library", "university", "research_lab"];
        let mut tally: std::collections::BTreeMap<String, usize> = Default::default();
        let mut finished_city_turns = 0usize;
        let mut idle_city_turns = 0usize;
        for seed in 0..4u64 {
            let mut g = Game::new(6, 60, 38, 89_000_000 + seed, 250, 6);
            g.game_speed = GameSpeed::Online;
            g.victory_conditions =
                crate::game::VictoryConditions::parse("science,culture,domination,score").unwrap();
            let mut me = AdvancedAi::new();
            me.enable_engine_repairs_universe();
            me.enable_research_tier_premium();
            me.enable_science_multiplier_payoff();
            let mut others = AdvancedAi::fleet(&g);
            while g.winner.is_none() && g.turn <= 250 {
                let pid = g.current;
                if pid == 0 {
                    me.take_turn(&mut g, pid);
                    if g.turn > 150 {
                        for cid in g.player_city_ids(0) {
                            let city = &g.cities[&cid];
                            if !g.city_has_district_family(city, campus) {
                                continue;
                            }
                            // "Finished" = every rung it can produce, it has.
                            let done = chain.iter().all(|rung| {
                                let name = Name::new(rung);
                                city.buildings.contains(&name)
                                    || !g.can_produce(
                                        0,
                                        cid,
                                        &crate::game::Item::Building { building: name },
                                    )
                            });
                            if !done {
                                continue;
                            }
                            finished_city_turns += 1;
                            match city.queue.first() {
                                None => idle_city_turns += 1,
                                Some(item) => {
                                    let family = match item {
                                        crate::game::Item::Unit { unit } => {
                                            format!("unit:{unit}")
                                        }
                                        crate::game::Item::Formation { unit, .. } => {
                                            format!("unit:{unit}")
                                        }
                                        crate::game::Item::Building { building } => {
                                            format!("building:{building}")
                                        }
                                        crate::game::Item::District { district, .. } => {
                                            format!("district:{district}")
                                        }
                                        crate::game::Item::Wonder { wonder, .. } => {
                                            format!("wonder:{wonder}")
                                        }
                                        crate::game::Item::Project { project } => {
                                            format!("project:{project}")
                                        }
                                        other => format!("{other:?}")
                                            .split_whitespace()
                                            .next()
                                            .unwrap_or("other")
                                            .to_ascii_lowercase(),
                                    };
                                    *tally.entry(family).or_default() += 1;
                                }
                            }
                        }
                    }
                } else {
                    others[pid].take_turn(&mut g, pid);
                }
                if g.winner.is_none() && g.current == pid {
                    let _ = g.apply(pid, &Action::EndTurn);
                }
            }
        }
        let mut ranked: Vec<(&String, &usize)> = tally.iter().collect();
        ranked.sort_by(|a, b| b.1.cmp(a.1));
        println!(
            "FINISHED finished_city_turns={finished_city_turns} idle={idle_city_turns} \
             ({:.1}% idle)",
            100.0 * idle_city_turns as f64 / finished_city_turns.max(1) as f64
        );
        for (family, count) in ranked.iter().take(14) {
            println!(
                "FINISHED   {:<40}{count:>6}  {:>5.1}%",
                family,
                100.0 * **count as f64 / finished_city_turns.max(1) as f64
            );
        }
    }
}

#[cfg(test)]
mod finished_city_ranking_probe {
    use super::*;

    /// Why does a finished research city never run `campus_research_grants`?
    ///
    /// The seven district projects are symmetric in the ruleset — cost 25,
    /// repeatable, 15% ongoing yield (Commercial Hub 30%), 10 Great Person
    /// points — yet over 1,113 finished-Campus city-turns past t150 the
    /// Theater and Commercial projects ran 4.5% and 3.6% of the time and the
    /// **Campus project ran ZERO**. Symmetric inputs and asymmetric output
    /// means the answer is in the valuation, so print it.
    #[test]
    #[ignore = "census, not an assertion; run explicitly with --nocapture"]
    fn what_does_a_finished_city_actually_rank_its_options() {
        let campus = crate::name!("campus");
        let mut g = Game::new(6, 60, 38, 89_000_008, 250, 6);
        g.game_speed = GameSpeed::Online;
        g.victory_conditions =
            crate::game::VictoryConditions::parse("science,culture,domination,score").unwrap();
        let mut me = AdvancedAi::new();
        me.enable_engine_repairs_universe();
        me.enable_research_tier_premium();
        me.enable_science_multiplier_payoff();
        let mut others = AdvancedAi::fleet(&g);
        let mut reported = false;
        while g.winner.is_none() && g.turn <= 250 {
            let pid = g.current;
            if pid == 0 {
                me.take_turn(&mut g, pid);
                if !reported && g.turn >= 200 {
                    let finished: Vec<u32> = g
                        .player_city_ids(0)
                        .into_iter()
                        .filter(|cid| {
                            let city = &g.cities[cid];
                            g.city_has_district_family(city, campus)
                                && city.buildings.contains(&crate::name!("research_lab"))
                        })
                        .collect();
                    if let Some(cid) = finished.first().copied() {
                        reported = true;
                        // The seven district projects, priced by the same
                        // function, in the same city, on the same turn.
                        let plan = me.assess(&g, 0);
                        println!(
                            "RANK city {cid} at t{} · production {:.0} · lane {:?}",
                            g.turn,
                            g.city_yields(cid).production,
                            plan.strategy
                        );
                        for project in [
                            "campus_research_grants",
                            "theater_square_festival",
                            "commercial_hub_investment",
                            "holy_site_prayers",
                            "industrial_zone_logistics",
                        ] {
                            let has_district = g.rules.projects[project]
                                .district
                                .map(|d| g.city_has_district_family(&g.cities[&cid], d))
                                .unwrap_or(true);
                            let value = me.district_project_value(&g, 0, cid, project, &plan);
                            println!(
                                "RANK   {value:>10.1}  {project:<28} district_here={has_district}"
                            );
                        }
                    }
                }
            } else {
                others[pid].take_turn(&mut g, pid);
            }
            if g.winner.is_none() && g.current == pid {
                let _ = g.apply(pid, &Action::EndTurn);
            }
        }
    }
}

#[cfg(test)]
mod great_scientist_probe {
    use super::*;

    /// Does the seat ever collect the Great People that multiply its chain?
    ///
    /// ★★ THE ONLY EMPIRE-WIDE SCIENCE MULTIPLIERS LEFT. Every science lever
    /// this campaign priced was per-city. The Great Scientists are not:
    ///
    /// ```text
    /// hypatia      era 1  cost   60  free_library      libraries_science    1
    /// omar_khayyam era 2  cost  120  free_library      libraries_science    1
    /// isaac_newton era 3  cost  240  free_university   universities_science 2
    /// charles_darwin era 4 cost 420  free_university   universities_science 2
    /// albert_einstein era 5 cost 660 modern_boosts     research_labs_science 4
    /// erwin_schrodinger era 6 cost 960                 research_labs_science 3
    /// ```
    ///
    /// `research_labs_science: 4` against the sixty Research Labs a census
    /// control finishes with is **+240 Science a turn**, an order of magnitude
    /// past anything in this bundle. `Game::city_yields` pays them through the
    /// `great_person:library_science` / `university_science` /
    /// `research_lab_science` counters.
    ///
    /// Before pricing anything: are they recruited at all, and does the empire
    /// hold the chain for them to multiply when they arrive?
    #[test]
    #[ignore = "census, not an assertion; run explicitly with --nocapture"]
    fn does_the_seat_ever_recruit_the_scientists_that_multiply_its_chain() {
        for seed in 0..4u64 {
            let mut g = Game::new(6, 60, 38, 91_000_000 + seed, 250, 6);
            g.game_speed = GameSpeed::Online;
            g.victory_conditions =
                crate::game::VictoryConditions::parse("science,culture,domination,score").unwrap();
            let mut me = AdvancedAi::new();
            me.enable_engine_repairs_universe();
            me.enable_research_tier_premium();
            me.enable_science_multiplier_payoff();
            let mut others = AdvancedAi::fleet(&g);
            let mut earned: Vec<(u32, String)> = Vec::new();
            let mut seen: std::collections::BTreeSet<String> = Default::default();
            while g.winner.is_none() && g.turn <= 250 {
                let pid = g.current;
                if pid == 0 {
                    me.take_turn(&mut g, pid);
                    for (name, spec) in g.rules.great_people.iter() {
                        if spec.kind != "scientist" {
                            continue;
                        }
                        if !seen.contains(name.as_str())
                            && g.players[0].great_people.iter().any(|held| held == name)
                        {
                            seen.insert(name.to_string());
                            earned.push((g.turn, name.to_string()));
                        }
                    }
                } else {
                    others[pid].take_turn(&mut g, pid);
                }
                if g.winner.is_none() && g.current == pid {
                    let _ = g.apply(pid, &Action::EndTurn);
                }
            }
            let counter = |key: &str| g.players[0].counters.get(key).copied().unwrap_or(0);
            let cities = g.player_city_ids(0);
            let holding = |name: &str| {
                cities
                    .iter()
                    .filter(|c| g.cities[c].buildings.contains(&Name::new(name)))
                    .count()
            };
            let labs = holding("research_lab");
            let per_lab = counter("great_person:research_lab_science");
            println!(
                "GPSCI seed {} scientists_earned={} {:?} · counters lib={} univ={} lab={} \
                 · chain lib={} univ={} lab={} · that lab counter is worth {} Science/turn \
                 · total gpp_scientist={:.0} · Science {:.1}",
                91_000_000 + seed,
                earned.len(),
                earned,
                counter("great_person:library_science"),
                counter("great_person:university_science"),
                per_lab,
                holding("library"),
                holding("university"),
                labs,
                per_lab as usize * labs,
                g.players[0].gpp.get("scientist").copied().unwrap_or(0.0),
                cities
                    .iter()
                    .map(|c| g.city_yields(*c).science)
                    .sum::<f64>(),
            );
        }
    }
}

#[cfg(test)]
mod patronage_reach_probe {
    use super::*;

    /// Does the PATRONAGE path — the one `great-person-effect-reach` reprices
    /// — ever actually decide anything?
    ///
    /// ⚠⚠ THE CHECK I SKIPPED HALF OF. `great_scientist_probe` established
    /// that the seat recruits one to three Great Scientists a game and that
    /// their counters pay 20–32 Science a turn, and I priced the patronage
    /// ranking on that. But a Great Person arriving on POINTS never passes
    /// through `advanced_great_people`'s candidate list at all, and the gene
    /// came back **byte-identical to control over twelve seeds**. The rule
    /// this bundle already carries is to ask whether the decision arises —
    /// and "the mechanism is used" is not the same question as "the code path
    /// I changed is used".
    ///
    /// This counts patronage actions, by currency, against the Great People
    /// that arrive without one.
    #[test]
    #[ignore = "census, not an assertion; run explicitly with --nocapture"]
    fn is_a_great_person_ever_bought_rather_than_earned() {
        for seed in 0..3u64 {
            let mut g = Game::new(6, 60, 38, 92_000_000 + seed, 250, 6);
            g.game_speed = GameSpeed::Online;
            g.victory_conditions =
                crate::game::VictoryConditions::parse("science,culture,domination,score").unwrap();
            let mut me = AdvancedAi::new();
            me.enable_engine_repairs_universe();
            let mut others = AdvancedAi::fleet(&g);
            let mut held = 0usize;
            let mut arrivals: Vec<(u32, String)> = Vec::new();
            let mut gold_spent_on_people = 0.0_f64;
            let mut faith_spent_on_people = 0.0_f64;
            while g.winner.is_none() && g.turn <= 250 {
                let pid = g.current;
                if pid == 0 {
                    let (gold_before, faith_before) = (g.players[0].gold, g.players[0].faith);
                    let before = g.players[0].great_people.len();
                    me.take_turn(&mut g, pid);
                    if g.players[0].great_people.len() > before {
                        for person in g.players[0].great_people.iter().skip(before) {
                            arrivals.push((g.turn, person.to_string()));
                        }
                        // A person that arrived on the same turn the treasury
                        // fell was bought; one that arrived with the treasury
                        // flat or rising was earned on points.
                        if g.players[0].gold < gold_before {
                            gold_spent_on_people += gold_before - g.players[0].gold;
                        }
                        if g.players[0].faith < faith_before {
                            faith_spent_on_people += faith_before - g.players[0].faith;
                        }
                    }
                    held = g.players[0].great_people.len();
                } else {
                    others[pid].take_turn(&mut g, pid);
                }
                if g.winner.is_none() && g.current == pid {
                    let _ = g.apply(pid, &Action::EndTurn);
                }
            }
            println!(
                "PATRONAGE seed {} great_people_held={held} arrivals={} \
                 gold_dropped_on_arrival={gold_spent_on_people:.0} \
                 faith_dropped_on_arrival={faith_spent_on_people:.0} \
                 · end gold={:.0} faith={:.0}",
                92_000_000 + seed,
                arrivals.len(),
                g.players[0].gold,
                g.players[0].faith,
            );
        }
    }
}
