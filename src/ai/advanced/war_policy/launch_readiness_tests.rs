use super::*;
use crate::ai::{AdvancedAi, VictoryTarget, GENES};

#[test]
#[ignore = "requires local recorded-game inputs"]
fn recorded_launch_requirements() {
    let root = std::env::var("CIVVIS_LAUNCH_SNAPSHOTS").unwrap();
    let input = std::fs::read_to_string(std::env::var("CIVVIS_LAUNCH_GENOMES").unwrap()).unwrap();
    let rows: Vec<serde_json::Value> = input
        .lines()
        .map(|s| serde_json::from_str(s).unwrap())
        .collect();
    let seat = rows
        .iter()
        .find(|r| r["kind"] == "game" && r["seed"] == 26091100 && r["seat"] == 0)
        .unwrap();
    for turn in [125, 175, 200] {
        let world: Game =
            serde_json::from_slice(&std::fs::read(format!("{root}/world-{turn}.json")).unwrap())
                .unwrap();
        let g = world.player_decision_view(0);
        let mut ai = AdvancedAi::new();
        ai.enable_engine_repairs_universe();
        for (tag, on) in rows[0]["genes"]
            .as_array()
            .unwrap()
            .iter()
            .zip(seat["genome"].as_str().unwrap().bytes())
        {
            let gene = GENES
                .iter()
                .find(|gene| Some(gene.tag) == tag.as_str())
                .unwrap();
            if (on == b'1') != gene.universe_on() {
                if on == b'1' {
                    (gene.enable)(&mut ai)
                } else {
                    (gene.disable)(&mut ai)
                }
            }
        }
        ai.retarget(VictoryTarget::Domination);
        for tag in [
            "early-conquest-opening",
            "siege-commitment",
            "siege-train",
            "domination-lane-hands-over",
            "recon-replacement",
        ] {
            (GENES.iter().find(|gene| gene.tag == tag).unwrap().enable)(&mut ai);
        }
        for tag in ["air-surge", "air-surge-2"] {
            (GENES.iter().find(|gene| gene.tag == tag).unwrap().disable)(&mut ai);
        }
        let mut view = g.clone();
        let mapped = view.units.keys().map(|id| (*id, i64::from(*id))).collect();
        crate::ai::player::plan_frame(&mut ai, &mut view, 0, &mapped);
        let plan = ai.plan.as_ref().unwrap();
        let target = plan.target_player.unwrap();
        let cid = plan.target_city.unwrap();
        let objective = g.cities[&cid].pos;
        let staged = ai.staged_campaign_units(&g, 0, target, objective);
        println!("AUDIT T{turn} {}: verdict {:?}, need {:?}, staged strength {}, local ratio {}, affordable {}, post-plan affordable {}, post-plan gold {} / {}",g.cities[&cid].name,ai.war_policy_declaration(&g,0,target,plan),ai.siege_requirement(&g,0,cid),AdvancedAi::campaign_strength_of(&g,&staged),ai.local_strength_ratio(&g,0,&staged,&[target],objective),ai.war_is_affordable(&g,0),ai.war_is_affordable(&view,0),view.players[0].gold,view.players[0].gold_per_turn);
        for uid in staged {
            let unit = &g.units[&uid];
            println!(
                "STAGED {} {} {:?} hp {} strength {}",
                uid,
                unit.kind,
                unit.pos,
                unit.hp,
                AdvancedAi::campaign_strength_of(&g, &[uid])
            );
        }
    }
}

#[test]
#[ignore = "full local diagnostic game"]
fn probe_ground_launches_with_preserved_controller_memory() {
    use crate::ai::{run_game_observed, Ai};
    use crate::game::{Action, GameOptions};
    use crate::setup::MapScript;
    use serde_json::{json, Value};
    struct Probe {
        ai: AdvancedAi,
        saved: usize,
    }
    impl Ai for Probe {
        fn uses_player_observation(&self) -> bool {
            self.ai.uses_player_observation()
        }
        fn take_turn(&mut self, g: &mut Game, pid: usize) {
            let mut candidate = None;
            if pid == 0 {
                if let Some(surge) = self
                    .ai
                    .air_surge_plan
                    .as_ref()
                    .filter(|p| !g.is_at_war(pid, p.target_player))
                {
                    let target = surge.target_player;
                    let phase = format!("{:?}", surge.phase);
                    let mut probe = self.ai.clone();
                    probe.air_surge_plan = None;
                    probe.air_surge_status = Default::default();
                    for tag in ["air-surge", "air-surge-2"] {
                        (GENES.iter().find(|gene| gene.tag == tag).unwrap().disable)(&mut probe);
                    }
                    let mut view = g.player_decision_view(pid);
                    let mapped = view.units.keys().map(|id| (*id, i64::from(*id))).collect();
                    let (_, begin) =
                        crate::ai::player::plan_frame(&mut probe, &mut view, pid, &mapped);
                    let orders: Vec<_> = view
                        .log
                        .since(begin)
                        .filter(|(seat, a)| {
                            *seat == pid
                                && matches!(
                                    a,
                                    Action::DeclareWar { .. }
                                        | Action::DeclareWarWithCasusBelli { .. }
                                        | Action::Denounce { .. }
                                )
                        })
                        .map(|(_, a)| format!("{a:?}"))
                        .collect();
                    if !orders.is_empty() {
                        candidate = Some((
                            g.turn,
                            target,
                            phase,
                            orders,
                            serde_json::to_vec(g).unwrap(),
                        ));
                    }
                }
            }
            self.ai.take_turn(g, pid);
            if let Some((turn, target, phase, orders, world)) = candidate {
                println!("COUNTERFACT T{turn} target {target} phase {phase} orders {orders:?} actual_war {}",g.is_at_war(pid,target));
                if !g.is_at_war(pid, target) && self.saved < 5 {
                    let root = std::env::var("CIVVIS_LAUNCH_AUDIT_OUTPUT").unwrap();
                    std::fs::write(format!("{root}/world-{turn}.json"), world).unwrap();
                    self.saved += 1;
                }
            }
        }
    }
    let seed = 26091100;
    let input = std::fs::read_to_string(std::env::var("CIVVIS_LAUNCH_GENOMES").unwrap()).unwrap();
    let rows: Vec<Value> = input
        .lines()
        .map(|s| serde_json::from_str(s).unwrap())
        .collect();
    let mut game = Game::new_with(GameOptions {
        speed: "online".into(),
        map_script: MapScript::Pangaea,
        randomize_civs: true,
        difficulty: "prince".into(),
        barbarian_difficulty: "deity".into(),
        ..GameOptions::new(4, 40, 26, seed, 500, 4)
    });
    game.native_competitions = true;
    let mix = crate::ai::player::parse_targets("domination,civvis").unwrap();
    let mut ais: Vec<_> = (0..game.players.len())
        .map(|pid| {
            let mut ai = AdvancedAi::new();
            if pid < 4 {
                let seat = rows
                    .iter()
                    .find(|r| r["kind"] == "game" && r["seed"] == seed && r["seat"] == pid)
                    .unwrap();
                ai.enable_engine_repairs_universe();
                for (tag, on) in rows[0]["genes"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .zip(seat["genome"].as_str().unwrap().bytes())
                {
                    let gene = GENES
                        .iter()
                        .find(|gene| Some(gene.tag) == tag.as_str())
                        .unwrap();
                    if (on == b'1') != gene.universe_on() {
                        if on == b'1' {
                            (gene.enable)(&mut ai)
                        } else {
                            (gene.disable)(&mut ai)
                        }
                    }
                }
                if let Some(target) = crate::ai::player::target_for(seed, pid, &mix) {
                    ai.retarget(target);
                    for tag in [
                        "early-conquest-opening",
                        "siege-commitment",
                        "siege-train",
                        "domination-lane-hands-over",
                        "recon-replacement",
                    ] {
                        (GENES.iter().find(|gene| gene.tag == tag).unwrap().enable)(&mut ai);
                    }
                }
            }
            Probe { ai, saved: 0 }
        })
        .collect();
    run_game_observed(&mut game, &mut ais, |g| {
        if g.turn % 50 == 0 {
            println!("TURN {}", g.turn);
        }
    });
    println!(
        "RESULT {}",
        json!({"seed":seed,"turn":game.turn,"winner":game.winner,"victory":game.victory_type,"capital_owners":game.cities.values().filter(|c|c.is_capital).map(|c|(c.original_owner,c.owner)).collect::<Vec<_>>() })
    );
}
