use civvis::ai::{run_game_observed, AdvancedAi, Ai, VictoryTarget};
use civvis::game::{Game, GameOptions};
use std::collections::BTreeSet;
use std::io::Write;

fn drain_journal(journal: &civvis::reasoning::Journal, cursor: &mut u64, out: &mut std::fs::File) {
    let delta = journal.since(*cursor);
    writeln!(
        out,
        "{}",
        serde_json::json!({"type":"journal_window","previous_cursor":cursor,
            "next_cursor":delta.cursor,"dropped_total":delta.dropped,
            "truncated_turns_total":delta.truncated_turns,"reset":delta.reset})
    )
    .unwrap();
    for thought in delta.thoughts {
        writeln!(out, "{}", serde_json::json!({"type":"planning_thought","id":thought.id,"turn":thought.turn,
            "player":thought.player,"headline":thought.headline,"detail":thought.detail,"focus":thought.focus})).unwrap();
    }
    *cursor = delta.cursor;
}

struct GrowthAi {
    base: AdvancedAi,
    enabled: bool,
    owned: BTreeSet<u32>,
}
impl Ai for GrowthAi {
    fn uses_player_observation(&self) -> bool {
        self.base.uses_player_observation()
    }
    fn take_turn(&mut self, g: &mut Game, pid: usize) {
        if self.enabled {
            for cid in std::mem::take(&mut self.owned) {
                g.players[pid].city_directives.remove(&cid);
            }
            if g.turn <= 60 {
                for cid in g.player_city_ids(pid) {
                    let c = &g.cities[&cid];
                    if c.pop >= 6
                        || g.city_housing(c) - (c.pop as f64) < 2.0
                        || g.city_amenity_surplus(c) < -1
                        || (c.last_attacked > 0 && g.turn.saturating_sub(c.last_attacked) < 4)
                    {
                        continue;
                    }
                    let danger = g.units.iter().any(|(uid, u)| {
                        u.owner != pid
                            && g.rules.units[&u.kind].class == "military"
                            && (g.players[u.owner].is_barbarian || g.is_at_war(pid, u.owner))
                            && civvis::hex::distance(c.pos, u.pos) <= 4
                            && g.unit_visible_to(*uid, pid)
                    });
                    if danger {
                        continue;
                    }
                    assert!(
                        !g.players[pid].city_directives.contains_key(&cid),
                        "do not replace another directive"
                    );
                    g.players[pid].city_directives.insert(
                        cid,
                        civvis::game::CityDirective {
                            emphasis: civvis::rules::Yields {
                                food: 2.0,
                                ..Default::default()
                            },
                            ..Default::default()
                        },
                    );
                    self.owned.insert(cid);
                }
            }
        }
        self.base.take_turn(g, pid);
    }
}

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
            !ais[0].city_strategy,
            "per-city priorities must be clear in this public baseline"
        );
        assert!(
            ais[0].wide_map_capacity,
            "live repair bundle must be enabled before play"
        );
        assert!(
            ais.iter().skip(1).all(|ai| !ai.wide_map_capacity),
            "stock rivals must retain their fixed configuration"
        );
        let setup = serde_json::json!({"seed":seed,"arm":arm,"difficulty":difficulty,"configuration":"public targeted live bridge, stock adaptive rivals, native simulation","wide_map_capacity":ais[0].wide_map_capacity,"weights_debug":format!("{:?}",ais[0].weights()),"handicaps":(0..4).map(|pid|g.handicap_yield_pct(pid)).collect::<Vec<_>>(),"deployment_tags":civvis::ai::deployment_treatments(),"young_city_growth":arm=="candidate","weights_vec":ais[0].weights().to_vec(),"actual_Firaxis_verification":false});
        std::fs::write(
            format!(
                "/tmp/civvis-production-citizen-growth-paired-{difficulty}-{seed}-{arm}-setup.json"
            ),
            serde_json::to_vec_pretty(&setup).unwrap(),
        )
        .unwrap();
        let mut detail = std::fs::File::create(format!(
            "/tmp/civvis-production-citizen-growth-paired-{difficulty}-{seed}-{arm}-cities.jsonl"
        ))
        .unwrap();
        let journal = civvis::reasoning::Journal::recording();
        ais[0].attach_journal(journal.handle());
        let mut ais: Vec<_> = ais
            .into_iter()
            .enumerate()
            .map(|(pid, base)| GrowthAi {
                base,
                enabled: pid == 0 && arm == "candidate",
                owned: BTreeSet::new(),
            })
            .collect();
        let mut cursor = 0;
        let mut thoughts = std::fs::File::create(format!(
            "/tmp/civvis-production-citizen-growth-paired-{difficulty}-{seed}-{arm}-thoughts.jsonl"
        ))
        .unwrap();
        let mut frames = std::fs::File::create(format!(
            "/tmp/civvis-production-citizen-growth-paired-{difficulty}-{seed}-{arm}-frames.jsonl"
        ))
        .unwrap();
        let mut log_cursor = 0;
        let mut sum = 0.0;
        run_game_observed(&mut g, &mut ais, |g| {
            drain_journal(&journal, &mut cursor, &mut thoughts);
            let _memo = g.query_memo();
            let cities = g.player_city_ids(0);
            let p: f64 = cities.iter().map(|c| g.city_yields(*c).production).sum();
            sum += p;
            let focal_actions: Vec<_> = g
                .log
                .iter()
                .skip(log_cursor)
                .filter(|(pid, _)| *pid == 0)
                .collect();
            let builders: Vec<_> = g
                .player_unit_ids(0)
                .into_iter()
                .filter(|uid| g.units[uid].kind == "builder")
                .map(|uid| &g.units[&uid])
                .collect();
            let city_summary: Vec<_> = cities.iter().map(|cid| { let c=&g.cities[cid]; serde_json::json!({"id":cid,"population":c.pop,"yields":g.city_yields(*cid),"queue":c.queue,"progress":c.production,"citizen_strategy":g.citizen_strategy(*cid),"directive":g.players[0].city_directives.get(cid)}) }).collect();
            writeln!(frames,"{}",serde_json::json!({"turn":g.turn,"actions":focal_actions,"builders":builders,"cities":city_summary})).unwrap();
            log_cursor = g.log.len();
            if [25, 50, 75].contains(&g.turn) {
                std::fs::write(
                    format!("/tmp/civvis-production-citizen-growth-paired-{difficulty}-{seed}-{arm}-view-t{}.json",g.turn),
                    serde_json::to_vec(&g.player_decision_view(0)).unwrap(),
                ).unwrap();
            }
            if [25, 50, 75, 100, 125, 150].contains(&g.turn) {
                let city_records: Vec<_> = cities.iter().map(|cid| {
                    let c=&g.cities[cid];
                    serde_json::json!({"id":cid,"name":c.name,"population":c.pop,"food_bank":c.food,"housing":g.city_housing(c),"amenities":g.city_amenity_surplus(c),"yields":g.city_yields(*cid),"queue":c.queue,"progress":c.production,"buildings":c.buildings,"districts":c.districts,"worked_tiles":g.city_citizen_plan(*cid).worked_tiles})
                }).collect();
                let rival_cities: Vec<_> = (1..4).flat_map(|pid| {
                    g.player_city_ids(pid).into_iter().map(move |cid| (pid,cid))
                }).map(|(pid,cid)| {
                    let c = &g.cities[&cid];
                    serde_json::json!({"owner":pid,"id":cid,"name":c.name,"population":c.pop,
                        "housing":g.city_housing(c),"amenities":g.city_amenity_surplus(c),
                        "yields":g.city_yields(cid),"queue":c.queue,"buildings":c.buildings,
                        "districts":c.districts,"worked_tiles":g.city_citizen_plan(cid).worked_tiles})
                }).collect();
                writeln!(detail,"{}",serde_json::json!({"seed":seed,"turn":g.turn,"gold":g.players[0].gold,"income":g.players[0].gold_per_turn,"bankruptcy_amenity_penalty":g.players[0].bankruptcy_amenity_penalty,"government":g.players[0].government,"policies":g.players[0].policies,"cities":city_records,"rival_cities":rival_cities})).unwrap();
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
        drain_journal(&journal, &mut cursor, &mut thoughts);
        let actions: Vec<_> = g.log.iter().collect();
        std::fs::write(
            format!(
                "/tmp/civvis-production-citizen-growth-paired-{difficulty}-{seed}-{arm}-actions.json"
            ),
            serde_json::to_vec(&actions).unwrap(),
        )
        .unwrap();
        let save = serde_json::to_vec(&g).unwrap();
        std::fs::write(
            format!(
                "/tmp/civvis-production-citizen-growth-paired-{difficulty}-{seed}-{arm}-final.json"
            ),
            save,
        )
        .unwrap();
        eprintln!(
            "{seed}: turn {}, winner {:?}, alive {}",
            g.turn, g.winner, g.players[0].alive
        );
    }
}
