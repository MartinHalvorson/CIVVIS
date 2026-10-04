use civvis::ai::{run_game_observed, AdvancedAi, Ai, VictoryTarget};
use civvis::game::{Game, GameOptions};
use std::collections::BTreeSet;
use std::io::Write;

// These are initial-turn decision-view diagnostics, before other focal units
// move or fight. Legal clone actions do not establish future capture safety.
fn builder_jobs(view: &Game) -> Vec<serde_json::Value> {
    let mut jobs = Vec::new();
    for cid in view.player_city_ids(0) {
        let city = &view.cities[&cid];
        for pos in view.city_citizen_plan(cid).worked_tiles {
            let tile = &view.map.tiles[&pos];
            if pos == city.pos || tile.owner_city != Some(cid) || tile.district.is_some() {
                continue;
            }
            for improvement in view.valid_improvements(0, pos) {
                if !view.rules.improvements[&improvement].builder_buildable {
                    continue;
                }
                let before = view.modeled_tile_yields(pos);
                let mut changed = view.clone();
                let target = changed.map.tiles.get_mut(&pos).unwrap();
                target.improvement = Some(improvement);
                target.pillaged = false;
                if changed.rules.improvements[&improvement].removes_feature {
                    target.feature = None;
                }
                let after = changed.modeled_tile_yields(pos);
                if after.production <= before.production {
                    continue;
                }
                jobs.push((cid, pos, improvement, after.production - before.production));
            }
        }
    }
    view.player_unit_ids(0).into_iter().filter_map(|uid| {
        let unit = &view.units[&uid];
        if unit.kind != "builder" { return None; }
        let adjacent: Vec<_> = jobs.iter().filter(|(_,pos,_,_)|view.wdist(unit.pos,*pos)<=1).map(|(cid,pos,improvement,gain)| {
            let mut trial = view.clone();
            let move_result = if *pos == unit.pos { Ok(()) } else {
                trial.apply(0, &civvis::game::Action::Move { unit:uid, to:*pos }).map(|_|())
            };
            let improve_result = if move_result.is_ok() {
                trial.apply(0, &civvis::game::Action::Improve { unit:uid, improvement:*improvement }).map(|_|())
            } else { Err("move refused".to_owned()) };
            let visible_hostiles: Vec<_> = view.units.iter().filter(|(id,u)| {
                view.is_at_war(0,u.owner) && view.unit_visible_to(**id,0)
                    && view.wdist(u.pos,*pos)<=3 && view.rules.units[&u.kind].class=="military"
            }).map(|(id,u)|serde_json::json!({"id":id,"kind":u.kind,"pos":u.pos})).collect();
            serde_json::json!({"city":cid,"pos":pos,"improvement":improvement,"tile_production_gain":gain,
                "clone_move_result":format!("{move_result:?}"),"clone_improve_result":format!("{improve_result:?}"),
                "visible_hostiles_within_three":visible_hostiles})
        }).collect();
        Some(serde_json::json!({"id":uid,"pos":unit.pos,"charges":unit.charges,
            "moves_left":unit.moves_left,"adjacent_worked_productive_jobs":adjacent}))
    }).collect()
}

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

// Separate first-frame projection. Its executor mirror is diagnostic only:
// it does not feed refusals or tactical information into the real controller.
struct RefusalProbe {
    inner: AdvancedAi,
    trace: Option<std::fs::File>,
}

impl Ai for RefusalProbe {
    fn uses_player_observation(&self) -> bool {
        self.inner.uses_player_observation()
    }

    fn take_turn(&mut self, g: &mut Game, pid: usize) {
        if pid == 0 && (60..=100).contains(&g.turn) {
            let mut projected = self.inner.clone();
            projected.attach_journal(civvis::reasoning::Journal::default());
            let mut view = g.player_decision_view(pid);
            let mapped = view.units.keys().map(|id| (*id, i64::from(*id))).collect();
            let (finishing, begin) =
                civvis::ai::player::plan_frame(&mut projected, &mut view, pid, &mapped);
            let mut trial = g.clone();
            trial.players[pid].citizen_food_bias = view.players[pid].citizen_food_bias;
            trial.players[pid].city_directives = view.players[pid].city_directives.clone();
            let ordinary: Vec<_> = view
                .log
                .since(begin)
                .filter(|(seat, action)| {
                    *seat == pid && !matches!(action, civvis::game::Action::EndTurn)
                })
                .map(|(_, a)| a.clone())
                .collect();
            let mut records = Vec::new();
            let mut refresh = false;
            let mut stopped = false;
            for (target, actions) in finishing.execution {
                if !trial.units.contains_key(&target) {
                    continue;
                }
                for action in actions {
                    let (record, change, stop) = diagnostic_apply(&mut trial, pid, &action);
                    records.push(record);
                    refresh |= change;
                    if stop {
                        stopped = true;
                        break;
                    }
                }
                if stopped {
                    break;
                }
            }
            let mut ordinary_attempts = 0;
            if !refresh && !stopped {
                for action in &ordinary {
                    let (record, _, stop) = diagnostic_apply(&mut trial, pid, action);
                    records.push(record);
                    ordinary_attempts += 1;
                    if stop {
                        stopped = true;
                        break;
                    }
                }
            }
            writeln!(self.trace.as_mut().unwrap(), "{}", serde_json::json!({
                "type":"separate_first_frame_projection", "turn":g.turn,
                "actual_log_len_before":g.log.len(), "planned_ordinary":ordinary,
                "execution_attempts":records, "ordinary_attempts":ordinary_attempts,
                "finishing_requires_refresh":refresh, "stopped":stopped,
                "limitation":"Separate clone projection, not a log of the real executor or later replanned frames"
            })).unwrap();
        }
        self.inner.take_turn(g, pid);
    }
}

fn diagnostic_apply(
    g: &mut Game,
    pid: usize,
    action: &civvis::game::Action,
) -> (serde_json::Value, bool, bool) {
    use civvis::game::{Action, Item};
    if g.current != pid || g.winner.is_some() {
        return (
            serde_json::json!({"action":action,"result":"terminal batch"}),
            false,
            true,
        );
    }
    let explored = g.players[pid].explored.len();
    let allocator = g.next_id;
    match g.apply(pid, action) {
        Err(error) => {
            let economic = match action {
                Action::BuyPlot { .. } => true,
                Action::Produce { item, .. } => !matches!(item, Item::District { .. }),
                Action::Trade { offer, request, .. } => {
                    offer.cities.is_empty()
                        && request.cities.is_empty()
                        && !offer.open_borders
                        && !request.open_borders
                }
                _ => false,
            };
            (
                serde_json::json!({"action":action,"result":error,"refused":true}),
                economic,
                !economic,
            )
        }
        Ok(_) => {
            let refresh = g.current != pid
                || g.winner.is_some()
                || g.players[pid].explored.len() != explored
                || g.next_id != allocator
                || matches!(
                    action,
                    Action::Attack { .. }
                        | Action::Ranged { .. }
                        | Action::CityStrike { .. }
                        | Action::EncampmentStrike { .. }
                        | Action::AirStrike { .. }
                        | Action::TheologicalAttack { .. }
                );
            (
                serde_json::json!({"action":action,"result":"accepted","refused":false}),
                refresh,
                false,
            )
        }
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    assert_eq!(args.len(), 3, "probe CONSUMED_SEED DIFFICULTY");
    let seed: u64 = args[1].parse().unwrap();
    let games: u64 = 1;
    let difficulty = &args[2];
    assert!(matches!(difficulty.as_str(), "emperor" | "deity"));
    let arm = "trace";
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
            format!("/tmp/civvis-production-builder-routing-projection-{difficulty}-{seed}-{arm}-setup.json"),
            serde_json::to_vec_pretty(&setup).unwrap(),
        )
        .unwrap();
        let mut detail = std::fs::File::create(format!(
            "/tmp/civvis-production-builder-routing-projection-{difficulty}-{seed}-{arm}-cities.jsonl"
        ))
        .unwrap();
        let journal = civvis::reasoning::Journal::recording();
        ais[0].attach_journal(journal.handle());
        let mut cursor = 0;
        let mut trace = std::fs::File::create(format!(
            "/tmp/civvis-production-builder-routing-projection-{difficulty}-{seed}-trace-rounds.jsonl"
        ))
        .unwrap();
        let mut ais: Vec<_> = ais
            .into_iter()
            .enumerate()
            .map(|(seat, inner)| RefusalProbe {
                inner,
                trace: if seat == 0 {
                    Some(
                        std::fs::File::create(format!(
                "/tmp/civvis-production-builder-routing-projection-{difficulty}-{seed}-trace-projections.jsonl"
            ))
                        .unwrap(),
                    )
                } else {
                    None
                },
            })
            .collect();
        let mut sum = 0.0;
        run_game_observed(&mut g, &mut ais, |g| {
            drain_journal(&journal, &mut cursor, &mut trace);
            if g.current == 0 && g.players[0].alive {
                let view = g.player_decision_view(0);
                writeln!(
                    trace,
                    "{}",
                    serde_json::json!({
                        "type":"round_start", "turn":g.turn, "actual_log_len":g.log.len(),
                        "builders":builder_jobs(&view),
                    })
                )
                .unwrap();
                if [50, 75].contains(&g.turn) {
                    std::fs::write(format!(
                        "/tmp/civvis-production-builder-routing-projection-{difficulty}-{seed}-trace-view-t{}.json", g.turn
                    ), serde_json::to_vec(&view).unwrap()).unwrap();
                }
            }
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
        drain_journal(&journal, &mut cursor, &mut trace);
        let actions: Vec<_> = g.log.iter().collect();
        std::fs::write(
            format!(
                "/tmp/civvis-production-builder-routing-projection-{difficulty}-{seed}-{arm}-actions.json"
            ),
            serde_json::to_vec(&actions).unwrap(),
        )
        .unwrap();
        let save = serde_json::to_vec(&g).unwrap();
        std::fs::write(
            format!("/tmp/civvis-production-builder-routing-projection-{difficulty}-{seed}-{arm}-final.json"),
            save,
        )
        .unwrap();
        eprintln!(
            "{seed}: turn {}, winner {:?}, alive {}",
            g.turn, g.winner, g.players[0].alive
        );
    }
}
