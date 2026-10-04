use civvis::ai::{run_game_observed, AdvancedAi, VictoryTarget};
use civvis::game::{Game, GameOptions, Item};
use std::collections::BTreeSet;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    assert_eq!(args.len(), 5, "probe START_SEED GAMES DIFFICULTY ARM");
    let seed: u64 = args[1].parse().unwrap();
    let games: u64 = args[2].parse().unwrap();
    let difficulty = &args[3];
    assert!(matches!(difficulty.as_str(), "emperor" | "deity"));
    assert!(matches!(args[4].as_str(), "control" | "candidate"));
    let candidate = args[4] == "candidate";
    println!("seed,turn,cities,production,rival_production,science,culture,gold,cumulative_production,alive,winner,population,granaries,housing_bound,housing_bound_without_granary,amenity_short_cities,builders,military_power,rival_cities,rival_population");
    for seed in seed..seed + games {
        let mut opts = GameOptions::new(4, 60, 38, seed, 150, 6);
        opts.map_script = civvis::setup::MapScript::Pangaea;
        opts.speed = "online".into();
        opts.difficulty = difficulty.into();
        opts.barbarian_difficulty = difficulty.into();
        opts.handicap_exempt = BTreeSet::from([0]);
        opts.civs = vec!["Gran Colombia".into()];
        opts.randomize_civs = true;
        let mut g = Game::new_with(opts);
        let mut ais = AdvancedAi::fleet(&g);
        ais[0] = AdvancedAi::targeting(VictoryTarget::Domination);
        ais[0].apply_gene_ledger();
        if candidate {
            ais[0].enable_treasury_at_work_2_2();
        }
        let mut sum = 0.0;
        run_game_observed(&mut g, &mut ais, |g| {
            let _memo = g.query_memo();
            let cities = g.player_city_ids(0);
            let p: f64 = cities.iter().map(|c| g.city_yields(*c).production).sum();
            sum += p;
            if [25, 50, 75, 100].contains(&g.turn) {
                let builders = g
                    .player_unit_ids(0)
                    .iter()
                    .filter(|u| g.units[u].kind == "builder")
                    .count();
                let queued = cities.iter().filter(|c| matches!(g.cities[c].queue.first(), Some(Item::Unit { unit }) if unit == "builder")).count();
                for cid in &cities {
                    let city = &g.cities[cid];
                    let yield_p = g.city_yields(*cid).production;
                    let eta = city.queue.first().map(|item| {
                        (g.item_cost_for_city(0, *cid, item) - city.production).max(0.0)
                            / (yield_p * g.item_prod_mult(0, *cid, Some(item))).max(0.001)
                    });
                    let worked = g.city_citizen_plan(*cid).worked_tiles;
                    let legal_worked = worked
                        .iter()
                        .filter(|pos| {
                            let tile = g.map.get(**pos).unwrap();
                            tile.improvement.is_none()
                                && g.valid_improvements(0, **pos).iter().any(|name| {
                                    matches!(name.as_str(), "mine" | "quarry" | "lumber_mill")
                                })
                        })
                        .count();
                    eprintln!("diagnostic seed={seed} turn={} city={cid} gold={:.1} gpt={:.1} field_builders={builders} queued_builders={queued} worked_legal_mine_quarry_lumber={legal_worked} production={yield_p:.1} queue={:?} eta_at_current_rate={eta:?} builder_gold_price={:?}", g.turn, g.players[0].gold, g.players[0].gold_per_turn, city.queue.first(), g.unit_purchase_cost(0, *cid, "builder", "gold"));
                }
            }
            if [25, 50, 75, 100, 125, 150].contains(&g.turn) {
                let rival = (1..4)
                    .map(|pid| {
                        let cities = g.player_city_ids(pid);
                        let production: f64 =
                            cities.iter().map(|c| g.city_yields(*c).production).sum();
                        let population = cities.iter().map(|c| g.cities[c].pop as u64).sum::<u64>();
                        (production, cities.len(), population)
                    })
                    .max_by(|a, b| a.0.total_cmp(&b.0))
                    .unwrap();
                let science: f64 = cities.iter().map(|c| g.city_yields(*c).science).sum();
                let culture: f64 = cities.iter().map(|c| g.city_yields(*c).culture).sum();
                let granary = civvis::name!("granary");
                let bound = |cid: &&u32| {
                    let city = &g.cities[*cid];
                    g.city_housing(city) - city.pop as f64 <= 1.0
                };
                println!(
                    "{seed},{},{},{p:.6},{:.6},{science:.6},{culture:.6},{:.6},{sum:.6},{},,{},{},{},{},{},{},{:.6},{},{}",
                    g.turn,
                    cities.len(),
                    rival.0,
                    g.players[0].gold,
                    g.players[0].alive,
                    cities.iter().map(|c| g.cities[c].pop as u64).sum::<u64>(),
                    cities.iter().filter(|c| g.cities[c].buildings.contains(&granary)).count(),
                    cities.iter().filter(bound).count(),
                    cities.iter().filter(|c| bound(c) && !g.cities[c].buildings.contains(&granary)).count(),
                    cities.iter().filter(|c| g.city_amenity_surplus(&g.cities[c]) < 0).count(),
                    g.player_unit_ids(0).iter().filter(|u| g.units[u].kind == "builder").count(),
                    g.military_power(0),
                    rival.1,
                    rival.2
                );
            }
        });
        eprintln!(
            "{seed}: turn {}, winner {:?}, alive {}",
            g.turn, g.winner, g.players[0].alive
        );
    }
}
