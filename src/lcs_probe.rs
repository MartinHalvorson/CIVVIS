//! PROBE ONLY, never committed: plays a stock world past its first result to its last civ.
use crate::game::{Item, PlayOnMode};
use crate::server::{probe_stock_params, Session};
use std::collections::BTreeMap;
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
    loop {
        s.step_quietly();
        let g = &s.game;
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
            emit(format!(
                "tick seed={seed} turn={} extra={} alive={} cities={cities:?} mil={mil:?} captures={captures} since_change={} quiet_max={quiet_max} secs={}",
                g.turn,
                g.turn - first,
                majors.len(),
                g.turn - last_change_turn,
                t0.elapsed().as_secs()
            ));
        }
        let quiet = g.turn - last_change_turn;
        let stalled =
            quiet >= 30 && g.turn >= last_diag + 20 && !done && dumps < diag_dumps;
        let reasoning = s.reasoning_json(cursor);
        cursor = reasoning["cursor"].as_u64().unwrap_or(cursor);
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
            if let Some(thoughts) = reasoning["thoughts"].as_array() {
                let mut shown = 0;
                for t in thoughts {
                    let head = t["headline"].as_str().unwrap_or("");
                    if t["topic"].as_str() == Some("military")
                        && head.starts_with("Siege of")
                        && shown < 10
                    {
                        shown += 1;
                        emit(format!(
                            "  think seed={seed} t={} p={} [{}] {head} :: {}",
                            t["turn"],
                            t["player"],
                            t["level"].as_str().unwrap_or(""),
                            t["detail"].as_str().unwrap_or("")
                        ));
                    }
                }
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
    /// One process per test under nextest, so each fixes its own switches.
    fn experiment(seed: u64, knobs: &str) {
        crate::lcs_knobs::set(knobs);
        let mut report = vec![format!("LCSREPORT seed={seed} knobs={knobs:?}")];
        super::run(seed, 300, 540, 2, |line| report.push(line));
        panic!("{}", report.join("\n"));
    }

    const COMMIT: &str = "patience=8,noabort=1,bill=1,nomuster=1";
    const TOTAL: &str = "bodies=4,patience=8,noabort=1,bill=1,nomuster=1";
    const TOTAL_NOHEAL: &str = "bodies=4,patience=8,noabort=1,bill=1,nomuster=1,noheal=1";

    #[test]
    fn lcs_base_19() {
        experiment(19, "");
    }
    #[test]
    fn lcs_commit_19() {
        experiment(19, COMMIT);
    }
    #[test]
    fn lcs_total_19() {
        experiment(19, TOTAL);
    }
    #[test]
    fn lcs_total_22() {
        experiment(22, TOTAL);
    }
    #[test]
    fn lcs_total_13() {
        experiment(13, TOTAL);
    }
    #[test]
    fn lcs_total_noheal_19() {
        experiment(19, TOTAL_NOHEAL);
    }
    #[test]
    fn lcs_economy_19() {
        experiment(19, "bodies=4");
    }
}
