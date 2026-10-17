use super::*;
use serde_json::json;

fn configured_agent(summary: &serde_json::Value) -> AdvancedAi {
    let mut agent = AdvancedAi::new();
    agent.enable_live_bridge_universe();
    let forced: Vec<&str> = summary["forced"]
        .as_array()
        .unwrap()
        .iter()
        .map(|tag| {
            crate::ai::GENES
                .iter()
                .find(|gene| Some(gene.tag) == tag.as_str())
                .unwrap()
                .tag
        })
        .collect();
    agent.apply_gene_ledger_with_forced_live(&forced);
    assert!(summary["withheld"].as_array().unwrap().is_empty());
    agent
}

fn under_ranged_fire(
    game: &Game,
    field: &mut super::super::battle_planner::DangerField,
    uid: u32,
    pos: Pos,
) -> bool {
    field.contributions(pos, uid).iter().any(|(source, blow)| {
        *blow > 0.0
            && source.is_some_and(|id| {
                id & super::super::battle_planner::STRUCTURE_SOURCE != 0
                    || game.units.get(&id).is_some_and(|unit| {
                        unit.owner != 0 && game.rules.units[unit.kind].has_ranged_attack()
                    })
            })
    })
}

#[test]
#[ignore = "read-only historical board diagnostic; requires an explicit external input directory"]
fn diagnose_original_native_opening_prefixes_without_host_actions() {
    let directory =
        std::path::PathBuf::from(std::env::var("CIVVIS_MUSTER_DIAGNOSTIC_DIR").unwrap());
    let summary: serde_json::Value =
        serde_json::from_slice(&std::fs::read(directory.join("summary.json")).unwrap()).unwrap();
    let mut boards = Vec::new();
    for turn in [146, 149, 150] {
        let events = directory.join(format!("t{turn}-f0/events.jsonl"));
        let snapshot = crate::mirror::snapshot_from_events(&events).unwrap();
        let state = crate::mirror::state_from_events(&events, None).unwrap();
        assert_eq!((state.turn, state.frame), (turn, 0));
        let mirror = crate::mirror::LiveMirror::new(&snapshot, &state, 4, 1, 250, 6);
        let game = &mirror.game;
        let city = game
            .cities
            .values()
            .find(|city| city.name == "Xanadu")
            .unwrap();
        assert_eq!(city.pos, (33, 22));
        let view = CityView::of(game, city.id).unwrap();
        let configured = configured_agent(&summary);
        let mut members = Vec::new();
        for (host_uid, uid) in &mirror.uid_of {
            let unit = &game.units[uid];
            let spec = &game.rules.units[unit.kind];
            if unit.owner != 0
                || spec.class != "military"
                || matches!(spec.domain.as_deref(), Some("air" | "sea"))
            {
                continue;
            }
            let limit = f64::from(unit.hp) * MUSTER_DANGER_SHARE;
            let road = game.dry_road_steps(*uid, city.pos, STAGING_FAR, MUSTER_ROAD_STEPS);
            let mut field = super::super::battle_planner::DangerField::with_reach(game, 0, true);
            if configured.shared_danger {
                field.share(game);
            }
            let risk_here = field.rotation_danger(unit.pos, *uid);
            let shot_here = under_ranged_fire(game, &mut field, *uid, unit.pos);
            let mut candidates = Vec::new();
            let mut eligible = Vec::new();
            for pos in game.reachable(*uid) {
                let risk = field.rotation_danger(pos, *uid);
                let shot = under_ranged_fire(game, &mut field, *uid, pos);
                let occupants = game.unit_ids_at(pos).to_vec();
                let dry = dry_stand(game, *uid, pos);
                let outside_fire_ring = game.wdist(pos, city.pos) > CITY_STRIKE_RANGE;
                let accepted = pos != unit.pos
                    && outside_fire_ring
                    && dry
                    && occupants.is_empty()
                    && risk <= limit
                    && !shot;
                if accepted {
                    eligible.push(pos);
                }
                candidates.push(json!({
                    "pos": pos,
                    "distance": game.wdist(pos, city.pos),
                    "dry_road_steps": road.get(&pos),
                    "risk": risk,
                    "under_ranged_fire": shot,
                    "occupants": occupants,
                    "dry_stand": dry,
                    "outside_city_fire_ring": outside_fire_ring,
                    "accepted": accepted,
                }));
            }
            let safe_road = (road.contains_key(&unit.pos)
                && game.wdist(unit.pos, city.pos) > MUSTER_BREACH_FAR
                && risk_here <= limit
                && !shot_here
                && !eligible.is_empty())
            .then(|| {
                muster_detour_steps(game, *uid, city.pos, |pos| {
                    field.rotation_danger(pos, *uid) <= limit
                        && !under_ranged_fire(game, &mut field, *uid, pos)
                })
            });
            let mut probe = game.speculative_clone();
            let mut agent = configured_agent(&summary);
            let acted = agent.muster_step(&mut probe, 0, *uid, &view);
            members.push(json!({
                "host_uid": host_uid,
                "uid": uid,
                "kind": unit.kind,
                "pos": unit.pos,
                "distance": game.wdist(unit.pos, city.pos),
                "hp": unit.hp,
                "moves": unit.moves_left,
                "embarked": game.is_embarked(unit),
                "risk_here": risk_here,
                "shot_here": shot_here,
                "limit": limit,
                "dry_road_steps_here": road.get(&unit.pos),
                "dry_road_tiles": road.len(),
                "safe_road_steps_here": safe_road.as_ref().and_then(|steps| steps.get(&unit.pos)),
                "safe_road_tiles": safe_road.as_ref().map(|steps| steps.len()),
                "eligible_endpoints": eligible,
                "reachable": candidates,
                "muster_result": acted,
                "pos_after": probe.units[uid].pos,
                "moves_after": probe.units[uid].moves_left,
            }));
        }
        boards.push(json!({
            "turn": turn,
            "frame": state.frame,
            "city": {"id": city.id, "pos": city.pos, "owner": city.owner, "name": city.name},
            "shared_danger": configured.shared_danger,
            "muster_walks_the_road": configured.muster_walks_the_road,
            "members": members,
        }));
    }
    let output = json!({
        "scope": "Cold reconstruction of three original opening prefixes, isolated muster method on speculative clones. Not a persistent decision replay, host execution, policy comparison, or strength result.",
        "boards": boards,
    });
    std::fs::write(
        directory.join("diagnostic.json"),
        serde_json::to_vec_pretty(&output).unwrap(),
    )
    .unwrap();
}
