use super::*;
use crate::ai::advanced::test_support::opt_in_off_in_both_controllers;
use crate::ai::advanced::VictoryTarget;

/// Three majors, one city each, all at peace and mutually known: seat 0 is
/// the Science seat, seats 1 and 2 its rivals. Seat 1 has landed a space
/// project, so it is the most pressing science threat.
fn board() -> Game {
    let mut g = Game::new_full(3, 30, 20, 79_208, 250, 0, false);
    for pid in 0..3 {
        let settler = g
            .player_unit_ids(pid)
            .into_iter()
            .find(|uid| g.units[uid].kind == "settler")
            .expect("each fixture major begins with a Settler");
        let position = g.units[&settler].pos;
        g.found_city_for(pid, position, None);
        for uid in g.player_unit_ids(pid) {
            g.remove_unit(uid);
        }
        g.players[pid].gold = 1_000.0;
    }
    g.current = 0;
    g.turn = 150;
    g.at_war.clear();
    g.record_contact(0, 1);
    g.record_contact(0, 2);
    g.players[1]
        .science_projects
        .insert("launch_earth_satellite".to_string());
    g
}

fn denier(gene: bool) -> AdvancedAi {
    let mut ai = AdvancedAi::targeting(VictoryTarget::Science);
    ai.enable_science_threat_denial();
    if gene {
        ai.enable_denounce_only_to_declare();
    }
    ai
}

fn denounced(g: &Game, by: usize, whom: usize) -> bool {
    g.players[by]
        .denounced_until
        .get(&whom)
        .is_some_and(|until| *until > g.turn)
}

#[test]
fn denounce_only_to_declare_is_opt_in() {
    opt_in_off_in_both_controllers("denounce-only-to-declare", |ai| ai.denounce_only_to_declare);
}

/// Off, the rung denounces the racer. On, it does not, and it counts the
/// denunciation it held back.
#[test]
fn the_threat_rung_holds_its_denunciation() {
    let mut off = board();
    assert_eq!(
        denier(false).science_threat_denunciation(&mut off, 0),
        Some(1),
        "off, the racer with a stage landed is denounced"
    );
    assert!(denounced(&off, 0, 1));

    let mut on = board();
    let ai = denier(true);
    assert!(
        ai.science_threats(&on, 0).iter().any(|t| t.rival == 1),
        "the fixture racer is a science threat"
    );
    assert_eq!(ai.science_threat_denunciation(&mut on, 0), None);
    assert!(!denounced(&on, 0, 1), "on, nobody is denounced");
    assert!(!denounced(&on, 0, 2));
    assert_eq!(
        on.players[0].counters.get("denunciations_withheld"),
        Some(&1),
        "the held-back denunciation is counted, so a screen row can see it"
    );
    assert_eq!(
        on.players[0].counters.get("denial_denunciations"),
        None,
        "the rung's own record stays empty"
    );
}

/// Nothing to denounce, nothing counted: a rival already denounced (or a
/// board without a threat) is not a withheld denunciation.
#[test]
fn only_a_legal_denunciation_is_counted() {
    let mut g = board();
    g.players[1].science_projects.clear();
    let ai = denier(true);
    assert!(ai.science_threats(&g, 0).iter().all(|t| t.stages == 0));
    assert_eq!(ai.science_threat_denunciation(&mut g, 0), None);
    assert_eq!(g.players[0].counters.get("denunciations_withheld"), None);

    let mut g = board();
    g.apply(0, &Action::Denounce { player: 1 }).unwrap();
    g.apply(0, &Action::Denounce { player: 2 }).unwrap();
    assert_eq!(ai.science_threat_denunciation(&mut g, 0), None);
    assert_eq!(g.players[0].counters.get("denunciations_withheld"), None);
}

/// The war opening keeps its denunciation: the Formal War clock still starts
/// when the war desk means to fight.
#[test]
fn the_war_opening_still_denounces() {
    let g = board();
    for target in [1, 2] {
        assert_eq!(
            denier(true).preferred_war_opening(&g, 0, target),
            denier(false).preferred_war_opening(&g, 0, target),
            "on and off open a war on seat {target} the same way"
        );
    }
}
