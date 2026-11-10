//! PROBE ONLY, never committed: plays a stock world past its first result to its last civ.
use crate::game::{Item, PlayOnMode};
use crate::server::{probe_stock_params, Session};
use std::collections::{BTreeMap, BTreeSet};
use std::time::Instant;

/// Play seed `seed` to its first result, press "One more turn", and report
/// how the war to the last civ goes for `extra_cap` turns or `wall_s` seconds.
pub fn run(seed: u64, extra_cap: u32, wall_s: u64, diag_dumps: u32, mut emit: impl FnMut(String)) {
    let mut s = Session::new(probe_stock_params(seed));
    let t0 = Instant::now();
    while !s.game.is_finished() {
        s.step_quietly();
    }
    let first = s.game.turn;
    emit(format!(
        "first_result seed={seed} turn={first} winner={:?} type={:?} secs={}",
        s.game.winner,
        s.game.victory_type,
        t0.elapsed().as_secs()
    ));
    assert!(s.play_on(PlayOnMode::LastCivStanding));
    let mut last = 0u32;
    let mut last_change_turn = s.game.turn;
    let mut owners: BTreeMap<u32, usize> =
        s.game.cities.iter().map(|(id, c)| (*id, c.owner)).collect();
    let mut captures = 0u32;
    let mut quiet_max = 0u32;
    let mut quiet_runs: Vec<u32> = Vec::new();
    let mut cursor = s.reasoning_json(0)["cursor"].as_u64().unwrap_or(0);
    let mut last_diag = 0u32;
    let mut dumps = 0u32;
    let mut seen: BTreeMap<usize, BTreeSet<u32>> = BTreeMap::new();
    let mut built: BTreeMap<usize, usize> = BTreeMap::new();
    let mut lost: BTreeMap<usize, usize> = BTreeMap::new();
    let mut recent: Vec<serde_json::Value> = Vec::new();
    loop {
        s.step_quietly();
        let g = &s.game;
        let mut soldiers: BTreeMap<usize, BTreeSet<u32>> = BTreeMap::new();
        for u in g.units.values() {
            if g.rules.units[u.kind].class == "military" {
                soldiers.entry(u.owner).or_default().insert(u.id);
            }
        }
        for (owner, now) in &soldiers {
            if let Some(before) = seen.get(owner) {
                *built.entry(*owner).or_default() += now.difference(before).count();
            }
        }
        for (owner, before) in &seen {
            let empty = BTreeSet::new();
            let now = soldiers.get(owner).unwrap_or(&empty);
            *lost.entry(*owner).or_default() += before.difference(now).count();
        }
        seen = soldiers;
        let majors: Vec<usize> = g
            .players
            .iter()
            .filter(|p| p.alive && !p.is_minor && !p.is_barbarian)
            .map(|p| p.id)
            .collect();
        let now: BTreeMap<u32, usize> = g.cities.iter().map(|(id, c)| (*id, c.owner)).collect();
        let changed = owners
            .iter()
            .filter(|(id, owner)| now.get(*id) != Some(*owner))
            .count() as u32;
        if changed > 0 {
            captures += changed;
            let run = g.turn - last_change_turn;
            if run >= 40 {
                quiet_runs.push(run);
            }
            last_change_turn = g.turn;
        }
        owners = now;
        quiet_max = quiet_max.max(g.turn - last_change_turn);
        let done = majors.len() <= 1 || g.is_finished();
        let out_of_time = g.turn >= first + extra_cap || t0.elapsed().as_secs() > wall_s;
        if g.turn >= last + 25 || done || out_of_time {
            last = g.turn;
            let cities: Vec<usize> = majors
                .iter()
                .map(|p| g.cities.values().filter(|c| c.owner == *p).count())
                .collect();
            let mil: Vec<usize> = majors
                .iter()
                .map(|p| {
                    g.units
                        .values()
                        .filter(|u| u.owner == *p && g.rules.units[u.kind].class == "military")
                        .count()
                })
                .collect();
            let made: Vec<usize> = majors
                .iter()
                .map(|p| built.get(p).copied().unwrap_or(0))
                .collect();
            let died: Vec<usize> = majors
                .iter()
                .map(|p| lost.get(p).copied().unwrap_or(0))
                .collect();
            built.clear();
            lost.clear();
            emit(format!(
                "tick seed={seed} turn={} extra={} alive={} cities={cities:?} mil={mil:?} built={made:?} lost={died:?} captures={captures} since_change={} quiet_max={quiet_max} secs={}",
                g.turn,
                g.turn - first,
                majors.len(),
                g.turn - last_change_turn,
                t0.elapsed().as_secs()
            ));
        }
        let quiet = g.turn - last_change_turn;
        let scheduled = dumps == 0 && g.turn >= first + 40;
        let stalled =
            (scheduled || quiet >= 30) && g.turn >= last_diag + 20 && !done && dumps < diag_dumps;
        let reasoning = s.reasoning_json(cursor);
        cursor = reasoning["cursor"].as_u64().unwrap_or(cursor);
        let this_turn = u64::from(g.turn);
        recent.retain(|t| t["turn"].as_u64().is_some_and(|turn| turn + 1 >= this_turn));
        if let Some(thoughts) = reasoning["thoughts"].as_array() {
            recent.extend(thoughts.iter().cloned());
        }
        if stalled {
            let g = &s.game;
            last_diag = g.turn;
            dumps += 1;
            let st = s.state();
            for p in &majors {
                let plan = &st["players"][*p]["ai_plan"];
                let forces: Vec<String> = plan["forces"]
                    .as_array()
                    .map(|f| {
                        f.iter()
                            .map(|x| {
                                format!(
                                    "{}/{}/{}u/r{}/s{}",
                                    x["domain"].as_str().unwrap_or("?"),
                                    x["posture"].as_str().unwrap_or("?"),
                                    x["units"],
                                    x["readiness"],
                                    x["strength_ratio"]
                                )
                            })
                            .collect()
                    })
                    .unwrap_or_default();
                let mut queues: BTreeMap<String, usize> = BTreeMap::new();
                for c in g.cities.values().filter(|c| c.owner == *p) {
                    let label = match c.queue.first() {
                        None => "idle".to_string(),
                        Some(Item::Unit { unit }) | Some(Item::Formation { unit, .. }) => {
                            if g.rules.units[*unit].class == "military" {
                                format!("unit:{}", unit.as_str())
                            } else {
                                format!("civ:{}", unit.as_str())
                            }
                        }
                        Some(Item::Building { .. }) => "building".to_string(),
                        Some(Item::District { .. }) => "district".to_string(),
                        Some(Item::Wonder { .. }) => "wonder".to_string(),
                        Some(Item::Repair { .. }) => "repair".to_string(),
                        Some(Item::Project { project }) => format!("project:{}", project.as_str()),
                        Some(Item::Product { .. }) => "product".to_string(),
                    };
                    *queues.entry(label).or_default() += 1;
                }
                emit(format!(
                    "stall seed={seed} turn={} seat={p} civ={:?} gold={:.0} gpt={:.0} power={:.0} strategy={} target={} city={} threat={} forces={forces:?} queues={queues:?}",
                    g.turn,
                    g.players[*p].civ,
                    g.players[*p].gold,
                    g.players[*p].gold_per_turn,
                    g.military_power(*p),
                    plan["strategy"],
                    plan["target_civ"],
                    plan["target_city"]["name"],
                    plan["threatened_city"]["name"]
                ));
            }
            let mut shown: BTreeMap<&str, u32> = BTreeMap::new();
            for t in &recent {
                if t["turn"].as_u64().is_none_or(|turn| turn + 1 != this_turn) {
                    continue;
                }
                let head = t["headline"].as_str().unwrap_or("");
                let topic = t["topic"].as_str().unwrap_or("");
                let (kind, cap) = match topic {
                    "strategy" if t["level"].as_str() == Some("strategy") => ("strategy", 8),
                    "cities" => ("cities", 14),
                    "military" if head.starts_with("Siege of") => ("siege", 10),
                    _ => continue,
                };
                let count = shown.entry(kind).or_default();
                if *count >= cap {
                    continue;
                }
                *count += 1;
                let detail: String = t["detail"]
                    .as_str()
                    .unwrap_or("")
                    .chars()
                    .take(360)
                    .collect();
                emit(format!(
                    "  think seed={seed} t={} p={} {kind} {head} :: {detail}",
                    t["turn"], t["player"]
                ));
            }
        }
        if done || out_of_time {
            let g = &s.game;
            emit(format!(
                "end seed={seed} turn={} extra={} alive={} captures={captures} quiet_max={quiet_max} quiet_runs={quiet_runs:?} finished={} winner={:?} type={:?} secs={}",
                g.turn,
                g.turn - first,
                majors.len(),
                g.is_finished(),
                g.winner,
                g.victory_type,
                t0.elapsed().as_secs()
            ));
            break;
        }
    }
}

#[cfg(test)]
mod experiments {
    /// Four baseline worlds at once; the report rides the panic into the CI log.
    #[test]
    fn lcs_base_matrix() {
        const KNOBS: &str = "conquest=1,objective=1,arm=1,bodies=4";
        crate::lcs_knobs::set(KNOBS);
        let handles: Vec<_> = [13u64, 14, 22, 23]
            .into_iter()
            .map(|seed| {
                std::thread::Builder::new()
                    .stack_size(32 << 20)
                    .spawn(move || {
                        let mut report = vec![format!(
                            "LCSREPORT seed={seed} knobs=\"{KNOBS}\" label=nosiege"
                        )];
                        super::run(seed, 400, 1380, 2, |line| report.push(line));
                        report.join("\n")
                    })
                    .expect("thread")
            })
            .collect();
        let reports: Vec<String> = handles
            .into_iter()
            .map(|handle| {
                handle
                    .join()
                    .unwrap_or_else(|_| "LCSREPORT thread panicked".to_string())
            })
            .collect();
        panic!("{}", reports.join("\n"));
    }
}
