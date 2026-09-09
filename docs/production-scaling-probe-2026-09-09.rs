use civvis::ai::{run_game_observed, AdvancedAi, VictoryTarget};
use civvis::game::{Game, GameOptions};
use civvis::setup::MapScript;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let start: u64 = args[1].parse().unwrap();
    let games: u64 = args[2].parse().unwrap();
    println!("seed,turn,seat,lane,cities,production,science,culture,gold,cumulative_production,alive,terminal");
    for seed in start..start + games {
        let mut options = GameOptions::new(6, 74, 46, seed, 250, 9);
        options.map_script = MapScript::Continents;
        options.speed = "online".into();
        options.difficulty = "emperor".into();
        options.barbarian_difficulty = "immortal".into();
        options.randomize_civs = true;
        let mut game = Game::new_with(options);
        let mut ais: Vec<_> = game
            .players
            .iter()
            .map(|p| {
                if p.id < 6 {
                    AdvancedAi::targeting(VictoryTarget::ALL[p.id])
                } else {
                    AdvancedAi::new()
                }
            })
            .collect();
        let mut cumulative = [0.0; 6];
        run_game_observed(&mut game, &mut ais, |g| {
            for pid in 0..6 {
                let cities = g.player_city_ids(pid);
                let production: f64 = cities.iter().map(|c| g.city_yields(*c).production).sum();
                cumulative[pid] += production;
                if g.turn % 25 == 0 {
                    let science: f64 = cities.iter().map(|c| g.city_yields(*c).science).sum();
                    let culture: f64 = cities.iter().map(|c| g.city_yields(*c).culture).sum();
                    println!("{seed},{},{pid},{},{},{production:.6},{science:.6},{culture:.6},{:.6},{:.6},{},false", g.turn, VictoryTarget::ALL[pid].as_str(), cities.len(), g.players[pid].gold, cumulative[pid], g.players[pid].alive);
                }
            }
        });
        for pid in 0..6 {
            let cities = game.player_city_ids(pid);
            let production: f64 = cities.iter().map(|c| game.city_yields(*c).production).sum();
            let science: f64 = cities.iter().map(|c| game.city_yields(*c).science).sum();
            let culture: f64 = cities.iter().map(|c| game.city_yields(*c).culture).sum();
            println!("{seed},{},{pid},{},{},{production:.6},{science:.6},{culture:.6},{:.6},{:.6},{},true", game.turn, VictoryTarget::ALL[pid].as_str(), cities.len(), game.players[pid].gold, cumulative[pid], game.players[pid].alive);
        }
        eprintln!(
            "seed {seed}: finished turn {}, winner {:?}",
            game.turn, game.winner
        );
    }
}
