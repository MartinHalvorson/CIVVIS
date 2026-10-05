use civvis::ai::{run_game_observed, AdvancedAi, VictoryTarget};
use civvis::game::{Game, GameOptions};
use civvis::setup::MapScript;
use std::collections::BTreeSet;
fn main() {
    let args: Vec<String> = std::env::args().collect();
    let seed: u64 = args[1].parse().unwrap();
    let games: u64 = args[2].parse().unwrap();
    let difficulty = &args[3];
    println!("seed,turn,cities,production,rival_production,science,culture,gold,cumulative_production,alive,winner,population,industrial_zones,workshops,builders,apprenticeship");
    for seed in seed..seed + games {
        let mut opts = GameOptions::new(4, 60, 38, seed, 150, 6);
        opts.map_script = MapScript::Pangaea;
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
        let mut sum = 0.0;
        run_game_observed(&mut g, &mut ais, |g| {
            let cities = g.player_city_ids(0);
            let p: f64 = cities.iter().map(|c| g.city_yields(*c).production).sum();
            sum += p;
            if [25, 50, 75, 100, 125, 150].contains(&g.turn) {
                let r = (1..4)
                    .map(|pid| {
                        g.player_city_ids(pid)
                            .iter()
                            .map(|c| g.city_yields(*c).production)
                            .sum::<f64>()
                    })
                    .fold(0.0, f64::max);
                let s: f64 = cities.iter().map(|c| g.city_yields(*c).science).sum();
                let c: f64 = cities.iter().map(|c| g.city_yields(*c).culture).sum();
                println!(
                    "{seed},{},{},{p:.6},{r:.6},{s:.6},{c:.6},{:.6},{sum:.6},{},,{},{},{},{},{}",
                    g.turn,
                    cities.len(),
                    g.players[0].gold,
                    g.players[0].alive,
                    cities.iter().map(|c| g.cities[c].pop as u64).sum::<u64>(),
                    cities
                        .iter()
                        .filter(|c| g.cities[c]
                            .districts
                            .contains_key(civvis::name!("industrial_zone")))
                        .count(),
                    cities
                        .iter()
                        .filter(|c| g.cities[c].buildings.contains(&civvis::name!("workshop")))
                        .count(),
                    g.player_unit_ids(0)
                        .iter()
                        .filter(|u| g.units[u].kind == "builder")
                        .count(),
                    g.players[0]
                        .techs
                        .contains(&civvis::name!("apprenticeship"))
                );
            }
        });
        eprintln!(
            "{seed}: turn {}, winner {:?}, alive {}",
            g.turn, g.winner, g.players[0].alive
        );
    }
}
