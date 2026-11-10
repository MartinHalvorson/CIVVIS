use super::*;
use crate::ai::advanced::launcher_war_every_lane::{DIPLOMATIC_LAUNCHER_EDGE, LANE_LAUNCHER_EDGE};
use crate::ai::advanced::test_support::opt_in_off_in_both_controllers;
use crate::game::Item;
use crate::name::Name;
use crate::reasoning::{Journal, Thought};
use crate::Pos;

const GENE: &str = "denial-war-needs-the-lane-edge";

/// Three majors, one city each, at peace and met: seat 0 denies, seat 1
/// races, seat 2 looks on. The science-denial fixture's board.
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
        g.players[pid].gold_per_turn = 10.0;
    }
    g.current = 0;
    g.turn = 60;
    g.at_war.clear();
    g.record_contact(0, 1);
    g.record_contact(0, 2);
    g
}

/// Seat 1 gets a seen Spaceport and a space race banked to the last turns.
fn racing_board() -> (Game, Pos) {
    let mut g = board();
    let cid = g.player_city_ids(1)[0];
    let centre = g.cities[&cid].pos;
    let pad = g.cities[&cid]
        .owned_tiles
        .iter()
        .copied()
        .find(|pos| {
            *pos != centre
                && g.map
                    .get(*pos)
                    .is_some_and(|tile| tile.district.is_none() && !g.rules.is_water(tile))
        })
        .expect("the fixture city owns a free land tile");
    let tile = g.map.tiles.get_mut(&pad).unwrap();
    tile.owner_city = Some(cid);
    tile.district = Some(crate::name!("spaceport"));
    tile.pillaged = false;
    tile.improvement = None;
    g.cities
        .get_mut(&cid)
        .unwrap()
        .districts
        .insert(crate::name!("spaceport"), pad);
    g.players[0].explored.insert(pad);
    for project in [
        "launch_earth_satellite",
        "launch_moon_landing",
        "launch_mars_colony",
        "exoplanet_expedition",
    ] {
        let item = Item::Project {
            project: Name::new(project),
        };
        let cost = g.item_cost_for_city(1, cid, &item);
        g.cities
            .get_mut(&cid)
            .unwrap()
            .production_progress
            .insert(format!("project:{project}"), cost);
    }
    (g, pad)
}

fn set_power(g: &mut Game, ours: f64, theirs: f64) {
    std::sync::Arc::make_mut(&mut g.observed_military_power).extend([(0, ours), (1, theirs)]);
}

/// A `target` seat whose denial war on seat 1 every other gate admits: the
/// first pass's denunciation has run its clock and two Warriors stand beside
/// the pad. Our power is the weak live case, 454 against 1236.
fn war_ready(target: VictoryTarget) -> (Game, AdvancedAi) {
    let mut ai = AdvancedAi::targeting(target);
    ai.enable_science_denial_war();
    let (mut g, pad) = racing_board();
    assert!(!ai.science_denial_war_diplomacy(&mut g, 0));
    g.turn += g.standard_duration(20);
    assert!(g.players[0].denounced_until[&1] > g.turn);
    let staging = g
        .wdisk(pad, 2)
        .into_iter()
        .find(|pos| {
            g.wdist(*pos, pad) == 2
                && g.map.get(*pos).is_some_and(|tile| !g.rules.is_water(tile))
                && g.city_at(*pos).is_none()
        })
        .expect("fixture has a reachable staging tile");
    g.spawn_test_unit("warrior", 0, staging);
    g.spawn_test_unit("warrior", 0, staging);
    set_power(&mut g, 454.0, 1236.0);
    (g, ai)
}

fn armed(mut ai: AdvancedAi) -> AdvancedAi {
    ai.enable_denial_war_needs_the_lane_edge();
    ai.base.journal = Journal::recording();
    ai
}

fn gene_lines(ai: &AdvancedAi) -> Vec<Thought> {
    ai.journal()
        .since(0)
        .thoughts
        .into_iter()
        .filter(|thought| thought.detail.contains(GENE))
        .collect()
}

fn denounced(g: &Game) -> bool {
    g.players[0]
        .denounced_until
        .get(&1)
        .is_some_and(|until| *until > g.turn)
}

#[test]
fn the_gene_is_opt_in_and_ships_off_in_both_controllers() {
    opt_in_off_in_both_controllers(GENE, |ai| ai.denial_war_needs_the_lane_edge);
}

#[test]
fn a_culture_seat_under_the_edge_opens_no_denial_war() {
    let (mut g, ai) = war_ready(VictoryTarget::Culture);
    let mut off = ai.clone();
    let mut control = g.clone();
    assert!(off.science_denial_war_diplomacy(&mut control, 0));
    assert!(control.is_at_war(0, 1), "control: the live declaration");

    let mut on = armed(ai);
    assert!(!on.science_denial_war_diplomacy(&mut g, 0));
    assert!(!g.is_at_war(0, 1));
    assert_eq!(on.denial_war, None);
    assert_eq!(g.players[0].counters.get("denial_wars"), None);
    let lines = gene_lines(&on);
    assert_eq!(lines.len(), 1, "one refusal, journalled");
    let civ = &g.players[1].civ;
    assert_eq!(
        lines[0].headline,
        format!("Culture keeps the peace with {civ}")
    );
    assert_eq!(
        lines[0].detail,
        "denial-war-needs-the-lane-edge: a denial war needs 1.2 times their steady power; \
         454 against 1236"
    );
}

#[test]
fn the_war_opens_at_the_lane_launcher_edge() {
    for (target, edge) in [
        (VictoryTarget::Culture, LANE_LAUNCHER_EDGE),
        (VictoryTarget::Religion, LANE_LAUNCHER_EDGE),
        (VictoryTarget::Science, LANE_LAUNCHER_EDGE),
        (VictoryTarget::Diplomacy, DIPLOMATIC_LAUNCHER_EDGE),
    ] {
        let at = edge * 100.0;
        for (ours, opens) in [(at - 0.01, false), (at, true), (at + 0.01, true)] {
            let (mut g, ai) = war_ready(target);
            set_power(&mut g, ours, 100.0);
            let mut ai = armed(ai);
            assert_eq!(
                ai.science_denial_war_diplomacy(&mut g, 0),
                opens,
                "{target:?} at {ours}"
            );
            assert_eq!(g.is_at_war(0, 1), opens, "{target:?} at {ours}");
            assert_eq!(gene_lines(&ai).is_empty(), opens, "{target:?} at {ours}");
        }
    }
}

/// The live failure: the war's opening denounced a declared friend on the
/// turn its friendship came up for renewal (140731Z Vietnam t134), and the
/// renewal then read "we have denounced them".
#[test]
fn a_lapsing_friend_is_neither_denounced_nor_declared_on() {
    let lapsing = || {
        let mut ai = AdvancedAi::targeting(VictoryTarget::Culture);
        ai.enable_science_denial_war();
        let (mut g, _) = racing_board();
        set_power(&mut g, 2000.0, 100.0);
        let turn = g.turn;
        g.players[0].friends_until.insert(1, turn);
        g.players[1].friends_until.insert(0, turn);
        assert!(!g.are_friends(0, 1), "the friendship ends this turn");
        (g, ai)
    };
    let (mut control, mut off) = lapsing();
    assert!(!off.science_denial_war_diplomacy(&mut control, 0));
    assert!(denounced(&control), "control: the friend is denounced");

    let (mut g, ai) = lapsing();
    let mut on = armed(ai);
    assert!(!on.science_denial_war_diplomacy(&mut g, 0));
    assert!(!denounced(&g), "the gene: no denunciation");
    assert!(!g.is_at_war(0, 1));
    let lines = gene_lines(&on);
    assert_eq!(lines.len(), 1);
    assert!(
        lines[0]
            .detail
            .contains("our friendship lapsed within 2 turns"),
        "{}",
        lines[0].detail
    );

    // Held through the renewal window, then the edge alone decides.
    g.turn += FRIENDSHIP_RENEW_WINDOW;
    assert!(!on.science_denial_war_diplomacy(&mut g, 0));
    assert!(!denounced(&g));
    g.turn += 1;
    assert!(!on.science_denial_war_diplomacy(&mut g, 0));
    assert!(denounced(&g), "past the window, at the edge: as before");
}

#[test]
fn a_friend_or_an_ally_is_spared_at_any_power() {
    let (g, ai) = war_ready(VictoryTarget::Culture);
    let ai = armed(ai);
    let mut strong = g.clone();
    set_power(&mut strong, 2000.0, 100.0);
    assert!(ai.denial_war_has_the_lane_edge(&strong, 0, 1));

    let mut friend = strong.clone();
    let turn = friend.turn;
    friend.players[0].friends_until.insert(1, turn + 10);
    assert!(
        !ai.denial_war_has_the_lane_edge(&friend, 0, 1),
        "declared friend"
    );

    let mut ally = strong.clone();
    ally.players[0].alliances.insert(
        1,
        crate::game::AllianceState {
            kind: "research".to_string(),
            points: 0.0,
            level: 1,
            ends: turn + 10,
        },
    );
    assert!(!ai.denial_war_has_the_lane_edge(&ally, 0, 1), "ally");

    let mut host = strong.clone();
    host.players[1]
        .observed_diplomatic_state
        .insert(0, "DECLARED_FRIEND".to_string());
    assert!(
        !ai.denial_war_has_the_lane_edge(&host, 0, 1),
        "the host still counts the friendship"
    );

    // The live mirror drops a friendship when it ends; the remembered
    // expiry still marks the renewal window.
    ai.friendship_expiry_seen.borrow_mut().insert(1, turn - 1);
    assert!(
        !ai.denial_war_has_the_lane_edge(&strong, 0, 1),
        "remembered lapse"
    );
    ai.friendship_expiry_seen.borrow_mut().insert(1, turn - 3);
    assert!(
        ai.denial_war_has_the_lane_edge(&strong, 0, 1),
        "an old lapse"
    );
}

#[test]
fn a_domination_seat_keeps_its_denial_war() {
    let (g, ai) = war_ready(VictoryTarget::Domination);
    let mut on = armed(ai);
    let mut friend = g.clone();
    friend.players[0].friends_until.insert(1, g.turn + 10);
    assert!(on.denial_war_has_the_lane_edge(&friend, 0, 1));
    let mut g = g;
    assert!(on.science_denial_war_diplomacy(&mut g, 0));
    assert!(g.is_at_war(0, 1));
    assert!(gene_lines(&on).is_empty());
}

/// Off, and on a Domination seat, the turn plays byte for byte as before.
#[test]
fn off_or_on_domination_the_turn_is_byte_identical() {
    for target in [
        VictoryTarget::Science,
        VictoryTarget::Culture,
        VictoryTarget::Religion,
        VictoryTarget::Diplomacy,
        VictoryTarget::Domination,
    ] {
        let (g, ai) = war_ready(target);
        let mut toggled = ai.clone();
        toggled.enable_denial_war_needs_the_lane_edge();
        toggled.disable_denial_war_needs_the_lane_edge();
        let arms: Vec<AdvancedAi> = if target == VictoryTarget::Domination {
            vec![toggled, armed(ai.clone())]
        } else {
            vec![toggled]
        };
        let mut control_game = g.clone();
        let mut control = ai.clone();
        let declared = control.science_denial_war_diplomacy(&mut control_game, 0);
        let expected = serde_json::to_value(&control_game).unwrap();
        for mut arm in arms {
            let mut game = g.clone();
            assert_eq!(
                arm.science_denial_war_diplomacy(&mut game, 0),
                declared,
                "{target:?}"
            );
            assert_eq!(arm.denial_war, control.denial_war, "{target:?}");
            assert!(
                serde_json::to_value(&game).unwrap() == expected,
                "{target:?}: the board differs"
            );
        }
    }
}
