//! Model-only feasibility diagnostic on frozen observational prefixes.
use civvis::game::{Action, ActionFamilies, Game};
use civvis::mirror::{host_major_seat_map, snapshot_from_events_at, state_from_events, LiveMirror};
use civvis::Pos;
use serde_json::{json, Value};
use std::path::Path;

fn theaters(g: &Game, rival: usize) -> Vec<Pos> {
    g.map
        .tiles
        .iter()
        .filter_map(|(_, tile)| {
            let cid = tile.owner_city?;
            if g.cities.get(&cid)?.owner != rival || tile.pillaged {
                return None;
            }
            let mut family = tile.district?;
            for _ in 0..g.rules.districts.len() {
                if let Some(parent) = g.rules.districts[family].replaces {
                    family = parent;
                } else {
                    break;
                }
            }
            (family == "theater_square").then_some(tile.pos)
        })
        .collect()
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    assert_eq!(args.len(), 3, "probe MANIFEST INDEX");
    let manifest: Value = serde_json::from_slice(&std::fs::read(&args[1]).unwrap()).unwrap();
    let index: usize = args[2].parse().unwrap();
    let case = &manifest["cases"][index];
    assert_eq!(case["status"], "prefix_frozen");
    let turn = case["turn"].as_u64().unwrap() as u32;
    let path = Path::new(case["path"].as_str().unwrap());
    let snapshot = snapshot_from_events_at(path, Some(turn)).unwrap();
    let state = state_from_events(path, Some(turn)).unwrap();
    assert_eq!(state.turn, turn);
    let players = state.seat.players.max(2);
    let turns = if state.seat.max_turns > 0 {
        state.seat.max_turns as u32
    } else {
        250
    };
    let live = LiveMirror::new(&snapshot, &state, players, 1, turns, 6);
    let g = &live.game;
    let seats = host_major_seat_map(&state, players);
    let mut reconstructed = Vec::new();
    let mut ready = Vec::new();
    for aircraft in case["aircraft"].as_array().unwrap() {
        let native = aircraft["id"].as_i64().unwrap();
        let model = live.uid_of.get(&native).copied();
        let unit = model.and_then(|uid| g.units.get(&uid));
        let usable = unit.is_some_and(|u| {
            u.owner == 0
                && u.hp >= 80
                && u.moves_left > 0.0
                && u.attacks_left > 0
                && g.rules.units[u.kind].domain.as_deref() == Some("air")
                && g.rules.units[u.kind].siege
        });
        reconstructed.push(json!({"native_id": native, "model_id": model,
            "ready": usable, "unit": unit.map(|u| json!({"hp": u.hp, "pos": u.pos,
                "moves": u.moves_left, "attacks": u.attacks_left, "kind": u.kind}))}));
        if usable {
            ready.push(model.unwrap());
        }
    }
    let diplomacy = g.legal_actions_within(0, ActionFamilies::DIPLOMACY);
    let rebase_actions: Vec<Action> = g
        .legal_actions_within(0, ActionFamilies::UNITS)
        .into_iter()
        .filter(|action| {
            matches!(action, Action::AirRebase { unit, .. }
            if ready.contains(unit))
        })
        .collect();
    let mut rivals = Vec::new();
    for racer in case["racers"].as_array().unwrap() {
        let host = racer["player"].as_u64().unwrap() as usize;
        let Some(&rival) = seats.get(&host) else {
            rivals.push(json!({"host": host, "status": "missing_seat"}));
            continue;
        };
        let targets = theaters(g, rival);
        let declarations: Vec<Action> = diplomacy
            .iter()
            .filter(|action| match action {
                Action::DeclareWar { player } | Action::DeclareWarWithCasusBelli { player, .. } => {
                    *player == rival
                }
                _ => false,
            })
            .cloned()
            .collect();
        let mut staging = Vec::new();
        for action in &rebase_actions {
            if let Action::AirRebase { unit, to } = action {
                let in_range: Vec<_> = targets
                    .iter()
                    .copied()
                    .filter(|target| g.wdist(*to, *target) <= g.unit_attack_range(*unit))
                    .collect();
                if in_range.is_empty() {
                    continue;
                }
                let mut copy = g.speculative_clone();
                let applied = copy.apply(0, action);
                staging.push(json!({"action": action, "apply_error": applied.err(),
                    "geometric_targets": in_range,
                    "scope": "legal current rebase; future sortie and survival unproven"}));
            }
        }
        let mut attempts = Vec::new();
        let mut war = g.speculative_clone();
        let opening = declarations.first();
        let war_error = opening.map(|action| war.apply(0, action)).map(|r| r.err());
        if opening.is_some() && war_error == Some(None) && war.is_at_war(0, rival) {
            for &unit in &ready {
                for &target in &targets {
                    let cid = war.map.get(target).unwrap().owner_city.unwrap();
                    let before_buildings = war.cities[&cid].pillaged_buildings.len();
                    let mut copy = war.speculative_clone();
                    let action = Action::AirPillage { unit, target };
                    let result = copy.apply(0, &action);
                    let hp = copy.units.get(&unit).map(|u| u.hp);
                    let pillaged = copy.map.get(target).unwrap().pillaged;
                    let layers = copy.cities[&cid].pillaged_buildings.len() - before_buildings;
                    attempts.push(json!({"action": action, "apply_error": result.err(),
                        "visible": war.player_can_see(0, target),
                        "distance": war.wdist(war.units[&unit].pos, target),
                        "range": war.unit_attack_range(unit), "surviving_hp": hp,
                        "district_pillaged": pillaged, "new_pillaged_buildings": layers,
                        "conservative_immediate_district_suppression": pillaged && hp.is_some_and(|h| h >= 60)}));
                }
            }
        }
        rivals.push(json!({"host": host, "model": rival, "targets": targets,
            "legal_declarations": declarations, "selected_opening": opening,
            "war_error": war_error, "attempts": attempts, "staging": staging}));
    }
    println!(
        "{}",
        json!({"index": index, "tag": case["tag"], "turn": turn,
        "scope": "model-only observational feasibility; no native acceptance or hidden-AA proof",
        "reconstructed_aircraft": reconstructed, "dropped_units": live.dropped_units,
        "unmapped": live.unmapped, "racers": rivals})
    );
}
