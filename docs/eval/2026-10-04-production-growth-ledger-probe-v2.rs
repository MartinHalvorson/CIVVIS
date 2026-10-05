use civvis::ai::{run_game_observed, AdvancedAi, VictoryTarget};
use civvis::game::{Game, GameOptions};
use std::collections::BTreeSet;
use std::io::Write;

// Counterfactual tile queries use a fresh clone, whose QueryCache::clone resets
// every memo. The actual observed world and its units are never modified.
fn improvement_gain(g: &Game, pos: (i32, i32), name: &str) -> civvis::rules::Yields {
    let before = g.modeled_tile_yields(pos);
    let mut after = g.clone();
    let tile = after.map.tiles.get_mut(&pos).unwrap();
    tile.improvement = Some(name.into());
    tile.pillaged = false;
    if after.rules.improvements[name].removes_feature {
        tile.feature = None;
    }
    let mut gain = after.modeled_tile_yields(pos);
    gain.add_scaled(before, -1.0);
    gain
}

fn economy(g: &Game, pid: usize) -> serde_json::Value {
    let cities: Vec<_> = g.player_city_ids(pid).into_iter().map(|cid| {
        let city = &g.cities[&cid];
        let plan = g.city_citizen_plan(cid);
        let jobs: Vec<_> = plan.worked_tiles.iter().filter_map(|pos| {
            let tile = g.map.get(*pos)?;
            let improvements: Vec<_> = if tile.owner_city == Some(cid) && tile.improvement.is_none() && tile.district.is_none() {
                g.valid_improvements(pid, *pos).into_iter().filter(|name| g.rules.improvements[name].builder_buildable && (!g.rules.improvements[name].removes_feature || tile.feature.is_none())).map(|name| serde_json::json!({"name": name, "gain": improvement_gain(g, *pos, name.as_str())})).collect()
            } else { Vec::new() };
            Some(serde_json::json!({"pos":pos,"terrain":tile.terrain,"hills":tile.hills,"feature":tile.feature,"resource":tile.resource,"improvement":tile.improvement,"pillaged":tile.pillaged,"owner_city":tile.owner_city,"yields":g.modeled_tile_yields(*pos),"legal_improvements":improvements}))
        }).collect();
        let mut production_weights=plan.strategy.weights;
        production_weights.production += 3.0;
        let mut food_weights=plan.strategy.weights;
        food_weights.food += 3.0;
        let workers: Vec<_> = g.player_unit_ids(pid).into_iter().filter_map(|uid| {
            let u=&g.units[&uid];
            (u.kind=="builder" && g.wdist(u.pos,city.pos)<=6).then_some(serde_json::json!({"id":uid,"pos":u.pos,"charges":u.charges,"hp":u.hp,"moves_left":u.moves_left}))
        }).collect();
        let mut all_work=Vec::new();
        for pos in &city.owned_tiles {
            let Some(tile)=g.map.get(*pos) else {continue};
            if *pos==city.pos || tile.improvement.is_some() || tile.district.is_some() || g.wdist(city.pos,*pos)>3 {continue}
            let gain=g.valid_improvements(pid,*pos).into_iter().filter(|name| g.rules.improvements[name].builder_buildable && (!g.rules.improvements[name].removes_feature || tile.feature.is_none())).map(|name| improvement_gain(g,*pos,name.as_str()).production).fold(0.0,f64::max);
            if gain>0.0 {all_work.push(serde_json::json!({"pos":pos,"gain":gain,"worked":plan.worked_tiles.contains(pos)}));}
        }
        serde_json::json!({"id":cid,"name":city.name,"pos":city.pos,"pop":city.pop,"food_bank":city.food,"growth_cost":g.growth_cost(city.pop),"housing":g.city_housing(city),"amenity_surplus":g.city_amenity_surplus(city),"loyalty":city.loyalty,"last_attacked":city.last_attacked,"hp":city.hp,"yields":g.city_yields(cid),"ledger":g.city_yield_ledger(cid),"citizens":plan,"queue":city.queue,"progress":city.production,"head_cost":city.queue.first().map(|item|g.item_cost_for_city(pid,cid,item)),"head_production_multiplier":g.item_prod_mult(pid,cid,city.queue.first()),"buildings":city.buildings,"districts":city.districts,"worked_jobs":jobs,"all_productive_work":all_work,"nearby_builders":workers,"builder_quote":g.unit_purchase_cost(pid,cid,"builder","gold"),"production_weight_counterfactual":g.city_yields_weighted(cid,production_weights),"food_weight_counterfactual":g.city_yields_weighted(cid,food_weights)})
    }).collect();
    let player = &g.players[pid];
    let deals: Vec<_> = g
        .active_trade_deals
        .iter()
        .filter(|d| d.ends > g.turn && (d.from == pid || d.to == pid))
        .collect();
    let contracted_gpt: f64 = deals
        .iter()
        .map(|d| {
            if d.from == pid {
                d.request.gold_per_turn - d.offer.gold_per_turn
            } else {
                d.offer.gold_per_turn - d.request.gold_per_turn
            }
        })
        .sum();
    let unit_maintenance: f64 = g
        .player_unit_ids(pid)
        .into_iter()
        .map(|uid| {
            let u = &g.units[&uid];
            let scale = match u.formation {
                0 => 1.0,
                1 => 1.5,
                _ => 2.0,
            };
            let surcharge = if g.rules.units[u.kind].class == "military" {
                g.policy_effect(pid, "unit_maintenance_surcharge")
            } else {
                0.0
            };
            (g.rules.units[u.kind].maintenance * scale
                - g.policy_effect(pid, "unit_maintenance_discount"))
            .max(0.0)
                + surcharge
        })
        .sum::<f64>()
        + g.spies.values().filter(|s| s.owner == pid).count() as f64
            * g.rules.units["spy"].maintenance;
    let units: Vec<_> = g
        .player_unit_ids(pid)
        .into_iter()
        .map(|uid| {
            let u = &g.units[&uid];
            serde_json::json!({"id":uid,"kind":u.kind,"pos":u.pos,"charges":u.charges,"hp":u.hp})
        })
        .collect();
    serde_json::json!({"pid":pid,"alive":player.alive,"civ":player.civ,"gold":player.gold,"income":player.gold_per_turn,"handicap":g.handicap_yield_pct(pid),"government":player.government,"policies":player.policies,"research":player.research,"research_progress":player.research_progress,"civic":player.civic,"civic_progress":player.civic_progress,"bankruptcy_amenity_penalty":player.bankruptcy_amenity_penalty,"activation_needs":player.live_great_person_activation_needs,"policy_yields":g.player_policy_yields(pid),"native_unit_maintenance":unit_maintenance,"contracted_gpt":contracted_gpt,"active_deals":deals,"techs":player.techs,"civics":player.civics,"wars":g.players.iter().enumerate().filter_map(|(other,p)|(p.alive && other!=pid && g.is_at_war(pid,other)).then_some(other)).collect::<Vec<_>>(),"cities":cities,"units":units})
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    assert_eq!(args.len(), 5, "probe START_SEED GAMES DIFFICULTY ARM");
    let seed: u64 = args[1].parse().unwrap();
    let games: u64 = args[2].parse().unwrap();
    let difficulty = &args[3];
    assert!(matches!(difficulty.as_str(), "emperor" | "deity"));
    assert!(matches!(args[4].as_str(), "stock" | "live" | "native"));
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
        match arm.as_str() {
            "stock" => {
                ais[0].apply_gene_ledger();
            }
            "live" => ais[0].enable_live_bridge(),
            "native" => {
                for (pid, ai) in ais.iter_mut().enumerate() {
                    if !g.players[pid].is_minor && !g.players[pid].is_barbarian {
                        ai.enable_native_deployment();
                    }
                }
            }
            _ => unreachable!(),
        }
        let flags: Vec<_> = ais.iter().take(4).enumerate().map(|(pid,ai)| serde_json::json!({"pid":pid,"wide_map_capacity":ai.wide_map_capacity,"war_economy":ai.war_economy})).collect();
        std::fs::write(format!("/tmp/civvis-production-growth-ledger-v2-{difficulty}-{seed}-{arm}-setup.json"),serde_json::to_vec_pretty(&serde_json::json!({"seed":seed,"arm":arm,"flags":flags,"deployment_tags":civvis::ai::deployment_treatments(),"setup_note":"stock reproduces earlier probe; live uses exact bridge bundle on focal seat, retaining stock rivals; native uses public native deployment helper for all majors. No private numeric genome. No inference about actual Firaxis outcomes."})).unwrap()).unwrap();
        let mut detail = std::fs::File::create(format!(
            "/tmp/civvis-production-growth-ledger-v2-{difficulty}-{seed}-{arm}-cities.jsonl"
        ))
        .unwrap();
        let mut sum = 0.0;
        run_game_observed(&mut g, &mut ais, |g| {
            let _memo = g.query_memo();
            let cities = g.player_city_ids(0);
            let p: f64 = cities.iter().map(|c| g.city_yields(*c).production).sum();
            sum += p;
            if [20, 25, 30, 40, 50, 60, 70, 75, 80, 90, 100].contains(&g.turn) {
                let players: Vec<_> = (0..4).map(|pid| economy(g, pid)).collect();
                writeln!(
                    detail,
                    "{}",
                    serde_json::json!({"seed":seed,"turn":g.turn,"players":players})
                )
                .unwrap();
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
                "/tmp/civvis-production-growth-ledger-v2-{difficulty}-{seed}-{arm}-actions.json"
            ),
            serde_json::to_vec(&actions).unwrap(),
        )
        .unwrap();
        let save = serde_json::to_vec(&g).unwrap();
        std::fs::write(
            format!("/tmp/civvis-production-growth-ledger-v2-{difficulty}-{seed}-{arm}-final.json"),
            save,
        )
        .unwrap();
        eprintln!(
            "{seed}: turn {}, winner {:?}, alive {}",
            g.turn, g.winner, g.players[0].alive
        );
    }
}
