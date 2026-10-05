use civvis::ai::{run_game_observed, AdvancedAi, VictoryTarget};
use civvis::game::{Game, GameOptions};
use std::collections::BTreeSet;
use std::io::Write;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    assert_eq!(args.len(), 5, "probe START_SEED GAMES DIFFICULTY ARM");
    let seed: u64 = args[1].parse().unwrap();
    let games: u64 = args[2].parse().unwrap();
    let difficulty = &args[3];
    assert!(matches!(difficulty.as_str(), "emperor" | "deity"));
    assert!(matches!(args[4].as_str(), "control" | "candidate"));
    let arm = &args[4];
    println!("seed,turn,cities,production,rival_production,science,culture,gold,cumulative_production,alive,winner,population,granaries,housing_bound,housing_bound_without_granary,amenity_short_cities,builders,military_power,rival_cities,rival_population,builder_gold_buys,productive_improvements");
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
        ais[0].enable_live_bridge();
        assert!(
            ais[0].wide_map_capacity,
            "live repair bundle must be enabled before play"
        );
        assert!(
            ais.iter().skip(1).all(|ai| !ai.wide_map_capacity),
            "stock rivals must retain their fixed configuration"
        );
        let setup = serde_json::json!({"seed":seed,"arm":arm,"difficulty":difficulty,"configuration":"public targeted live bridge, stock adaptive rivals, native simulation","wide_map_capacity":ais[0].wide_map_capacity,"weights_debug":format!("{:?}",ais[0].weights()),"handicaps":(0..4).map(|pid|g.handicap_yield_pct(pid)).collect::<Vec<_>>(),"deployment_tags":civvis::ai::deployment_treatments(),"actual_Firaxis_verification":false});
        std::fs::write(
            format!("/tmp/civvis-production-growth-capacity-{difficulty}-{seed}-{arm}-setup.json"),
            serde_json::to_vec_pretty(&setup).unwrap(),
        )
        .unwrap();
        let mut detail = std::fs::File::create(format!(
            "/tmp/civvis-production-growth-capacity-{difficulty}-{seed}-{arm}-cities.jsonl"
        ))
        .unwrap();
        let mut sum = 0.0;
        run_game_observed(&mut g, &mut ais, |g| {
            let _memo = g.query_memo();
            let cities = g.player_city_ids(0);
            let p: f64 = cities.iter().map(|c| g.city_yields(*c).production).sum();
            sum += p;
            if [25, 50, 75, 100, 125, 150].contains(&g.turn) {
                let city_records: Vec<_> = cities.iter().map(|cid| {
                    let c=&g.cities[cid];
                    serde_json::json!({"id":cid,"name":c.name,"population":c.pop,"food_bank":c.food,"housing":g.city_housing(c),"amenities":g.city_amenity_surplus(c),"yields":g.city_yields(*cid),"queue":c.queue,"progress":c.production,"buildings":c.buildings,"districts":c.districts})
                }).collect();
                writeln!(detail,"{}",serde_json::json!({"seed":seed,"turn":g.turn,"gold":g.players[0].gold,"income":g.players[0].gold_per_turn,"bankruptcy_amenity_penalty":g.players[0].bankruptcy_amenity_penalty,"government":g.players[0].government,"policies":g.players[0].policies,"cities":city_records})).unwrap();
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
                let builder_gold_buys = g.log.iter().filter(|(owner, action)| *owner == 0 && matches!(action, civvis::game::Action::Buy { unit, currency, .. } if unit == "builder" && currency == "gold")).count();
                let productive_improvements = g.log.iter().filter(|(owner, action)| *owner == 0 && matches!(action, civvis::game::Action::Improve { improvement, .. } if g.rules.improvements[improvement].yields.production > 0.0)).count();
                println!(
                    "{seed},{},{},{p:.6},{:.6},{science:.6},{culture:.6},{:.6},{sum:.6},{},,{},{},{},{},{},{},{:.6},{},{},{builder_gold_buys},{productive_improvements}",
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
        let actions: Vec<_> = g.log.iter().collect();
        std::fs::write(
            format!(
                "/tmp/civvis-production-growth-capacity-{difficulty}-{seed}-{arm}-actions.json"
            ),
            serde_json::to_vec(&actions).unwrap(),
        )
        .unwrap();
        let save = serde_json::to_vec(&g).unwrap();
        std::fs::write(
            format!("/tmp/civvis-production-growth-capacity-{difficulty}-{seed}-{arm}-final.json"),
            save,
        )
        .unwrap();
        eprintln!(
            "{seed}: turn {}, winner {:?}, alive {}",
            g.turn, g.winner, g.players[0].alive
        );
    }
}
